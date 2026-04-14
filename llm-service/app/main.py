import time
import uuid
from enum import Enum
from typing import Optional

from config import CODE_RETRIES_COUNT, GENERATION_MODEL, OLLAMA_URL, CONFIRM_WORD
from fastapi import FastAPI, HTTPException
from json_input_parser import ParseError, extract_context_and_clean_task
from pipeline import GenerationPipeline
from pydantic import BaseModel
from sandbox_client import extract_validation_feedback, send_code_for_validation

# ---------------------------------------------------------------------------
# App
# ---------------------------------------------------------------------------
app = FastAPI(title="LLM Generation Service", version="0.1.0")


# ---------------------------------------------------------------------------
# Session state machine
# ---------------------------------------------------------------------------
class SessionState(str, Enum):
    GENERATING_PLAN = "generating_plan"
    AWAITING_PLAN_CONFIRMATION = "awaiting_plan_confirmation"
    GENERATING_CODE = "generating_code"
    AWAITING_CODE_APPROVAL = "awaiting_code_approval"
    DONE = "done"


class SessionData:
    __slots__ = (
        "state",
        "user_task",
        "context",
        "plan",
        "plan_revision_count",
        "current_code",
        "code_revision_count",
        "sandbox_feedback",
        "created_at",
    )

    def __init__(self, task: str):
        self.state = SessionState.GENERATING_PLAN
        self.user_task = task
        self.context = None
        self.plan = ""
        self.plan_revision_count = 0
        self.current_code = ""
        self.code_revision_count = 0
        self.sandbox_feedback = ""
        self.created_at = time.time()


# In-memory session store (local-only, no external deps)
sessions: dict[str, SessionData] = {}

# Single pipeline instance — shared across sessions
pipeline = GenerationPipeline(GENERATION_MODEL, OLLAMA_URL, max_retries=2)


# ---------------------------------------------------------------------------
# Request / Response models
# ---------------------------------------------------------------------------
class GenerateRequest(BaseModel):
    session_id: Optional[str] = None
    task: str = ""
    user_response: str = ""  # "Подтвердить" or feedback/corrections


class GenerateResponse(BaseModel):
    session_id: str
    state: str
    plan: Optional[str] = None
    code: Optional[str] = None
    sandbox_feedback: Optional[str] = None
    message: str = ""


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
def _get_or_create_session(req: GenerateRequest) -> tuple[str, SessionData, bool]:
    """Return (session_id, session, is_new)."""
    if req.session_id and req.session_id in sessions:
        return req.session_id, sessions[req.session_id], False
    if not req.task:
        raise HTTPException(
            status_code=400, detail="task is required for a new session"
        )
    sid = req.session_id or str(uuid.uuid4())
    sess = SessionData(req.task)
    sessions[sid] = sess
    return sid, sess, True


def _strip_code_block(text: str) -> str:
    """Extract raw Lua code from ```lua ... ``` blocks."""
    if "```lua" in text:
        start = text.index("```lua") + len("```lua")
        end = text.find("```", start)
        if end == -1:
            return text[start:].strip()
        return text[start:end].strip()
    if "```" in text:
        start = text.index("```") + len("```")
        end = text.find("```", start)
        if end == -1:
            return text[start:].strip()
        return text[start:end].strip()
    return text.strip()


# ---------------------------------------------------------------------------
# Core state machine
# ---------------------------------------------------------------------------
async def _handle_plan_generation(session: SessionData) -> GenerateResponse:
    """Step 1 — generate initial plan."""
    plan = await pipeline._generate_plan(session.user_task)
    session.plan = plan
    session.state = SessionState.AWAITING_PLAN_CONFIRMATION
    return GenerateResponse(
        session_id="",  # filled by caller
        state=session.state,
        plan=plan,
        message="План сгенерирован. Подтвердите или укажите исправления.",
    )


async def _handle_plan_revision(
    session: SessionData, user_feedback: str
) -> GenerateResponse:
    """Step 2 — revise plan based on user feedback (loopable)."""
    session.plan_revision_count += 1
    # Feed previous plan + user corrections back into architect
    refined_plan = await pipeline._generate_plan(
        f"Предыдущий план:\n{session.plan}\n\nИсправления от пользователя:\n{user_feedback}\n\nОбнови план с учётом исправлений.",
    )
    session.plan = refined_plan
    session.state = SessionState.AWAITING_PLAN_CONFIRMATION
    return GenerateResponse(
        session_id="",
        state=session.state,
        plan=refined_plan,
        message=f"План обновлён (версия {session.plan_revision_count}). Подтвердите или укажите исправления.",
    )


async def _handle_code_generation(session: SessionData) -> GenerateResponse:
    """Step 3 — generate code from confirmed plan, run sandbox, run Ollama critic, then return to user."""
    session.state = SessionState.GENERATING_CODE
    code = await pipeline._generate_code(session.plan, session.user_task)
    raw_code = _strip_code_block(code)
    session.current_code = raw_code
    session.code_revision_count = 0
    session.sandbox_feedback = ""

    # --- Pass 1: Rust sandbox (interpretation errors) ---
    try:
        sandbox_resp = await send_code_for_validation(raw_code, session.context)
        sandbox_feedback = extract_validation_feedback(sandbox_resp)
    except Exception as exc:
        sandbox_feedback = str(exc)

    if sandbox_feedback != True:
        # Sandbox error — auto-fix loop
        for attempt in range(1, CODE_RETRIES_COUNT + 1):
            try:
                fixed_code = await pipeline._generate_code(
                    session.plan,
                    session.user_task,
                    previous_code=session.current_code,
                    critic_feedback=f"Ошибка песочницы: {sandbox_feedback}",
                )
                raw_fixed = _strip_code_block(fixed_code)
                sandbox_resp2 = await send_code_for_validation(
                    raw_fixed, session.context
                )
                fb2 = extract_validation_feedback(sandbox_resp2)
                if fb2 is True:
                    session.current_code = raw_fixed
                    sandbox_feedback = True
                    break
                else:
                    session.current_code = raw_fixed
                    sandbox_feedback = fb2
            except Exception as exc:
                sandbox_feedback = str(exc)

        if sandbox_feedback is not True:
            # Exhausted sandbox retries — still proceed to Ollama critic
            pass

    # --- Pass 2: Ollama critic (logic, security, performance) ---
    critic_result = await pipeline._critique_code(session.current_code)

    # Combine feedback for display
    combined_feedback = ""
    if sandbox_feedback is not True:
        combined_feedback += f"Sandbox: {sandbox_feedback}\n"
    if critic_result.upper() != CONFIRM_WORD:
        combined_feedback += f"Critic: {critic_result}"

    session.state = SessionState.AWAITING_CODE_APPROVAL
    return GenerateResponse(
        session_id="",
        state=session.state,
        code=session.current_code,
        sandbox_feedback=combined_feedback or None,
        message="Код прошёл проверки. Подтвердите или укажите исправления.",
    )


async def _handle_code_revision(session: SessionData, user_feedback: str) -> GenerateResponse:
    """Step 4 — revise code based on user feedback (loopable), then sandbox + Ollama critic."""
    session.code_revision_count += 1
    revised_code = await pipeline._generate_code(
        session.plan,
        session.user_task,
        previous_code=session.current_code,
        critic_feedback=f"Замечания пользователя:\n{user_feedback}",
    )
    raw_revised = _strip_code_block(revised_code)
    session.current_code = raw_revised

    # --- Pass 1: Rust sandbox ---
    sandbox_feedback = ""
    try:
        sandbox_resp = await send_code_for_validation(raw_revised, session.context)
        fb = extract_validation_feedback(sandbox_resp)
        if fb is not True:
            sandbox_feedback = str(fb)
    except Exception as exc:
        sandbox_feedback = str(exc)

    # --- Pass 2: Ollama critic ---
    critic_feedback = ""
    try:
        critic_result = await pipeline._critique_code(raw_revised)
        if critic_result:
            critic_result = critic_result.strip()
            if critic_result != CONFIRM_WORD:
                critic_feedback = critic_result
    except Exception as exc:
        critic_feedback = str(exc)

    msg = f"Код обновлён (версия {session.code_revision_count})."
    issues = []
    if sandbox_feedback is not True:
        issues.append(f"песочница: {sandbox_feedback}")
    if critic_feedback:
        issues.append(f"критик: {critic_feedback}")
    if issues:
        msg += " Замечания — " + "; ".join(issues) + "."
    else:
        msg += " Все проверки пройдены."
    msg += " Подтвердите или укажите исправления."

    return GenerateResponse(
        session_id="",
        state=session.state,
        code=raw_revised,
        sandbox_feedback=sandbox_feedback or None,
        message=msg,
    )


# ---------------------------------------------------------------------------
# Endpoint
# ---------------------------------------------------------------------------
@app.post("/generate", response_model=GenerateResponse)
async def generate(req: GenerateRequest):
    sid, session, is_new = _get_or_create_session(req)

    if is_new:
        try:
            clean_task, context = extract_context_and_clean_task(req.task)
            session.user_task = req.task
            session.context = context
        except ParseError as e:
            session.user_task = req.task  # fallback to raw task
            session.context = {"wf": {"vars": {}, "initVariables": {}}}
    # ── First call: generate plan ──────────────────────────────────────
    if session.state == SessionState.GENERATING_PLAN:
        result = await _handle_plan_generation(session)

    # ── User is confirming / revising the plan ────────────────────────
    elif session.state == SessionState.AWAITING_PLAN_CONFIRMATION:
        if req.user_response.strip() == "Подтвердить":
            # Plan approved — move to code generation
            result = await _handle_code_generation(session)
        else:
            # Revise plan (loop — user can repeat indefinitely)
            result = await _handle_plan_revision(session, req.user_response)

    # ── Code was just generated (auto sandbox pass), waiting approval ─
    elif session.state == SessionState.AWAITING_CODE_APPROVAL:
        if req.user_response.strip() == "Подтвердить":
            session.state = SessionState.DONE
            result = GenerateResponse(
                session_id=sid,
                state=session.state,
                code=session.current_code,
                message="Код одобрен. Генерация завершена.",
            )
        else:
            # Revise code based on user feedback (loop — repeat indefinitely)
            result = await _handle_code_revision(session, req.user_response)

    # ── Done — any further call returns the approved code ─────────────
    elif session.state == SessionState.DONE:
        result = GenerateResponse(
            session_id=sid,
            state=session.state,
            code=session.current_code,
            message="Код уже одобрен. Используйте тот же session_id для получения результата.",
        )

    # ── Should not happen ─────────────────────────────────────────────
    else:
        raise HTTPException(
            status_code=500, detail=f"Unexpected state: {session.state}"
        )

    # Fill session_id into response
    result.session_id = sid
    return result


# ---------------------------------------------------------------------------
# Health
# ---------------------------------------------------------------------------
@app.get("/health")
async def health():
    return {
        "status": "ok",
        "active_sessions": len(sessions),
        "pipeline_model": GENERATION_MODEL,
    }
