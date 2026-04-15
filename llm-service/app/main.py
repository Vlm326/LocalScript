import time
import uuid
import json
from enum import Enum
from typing import Optional
from rag_func import build_rag_context

from config import (
    CODE_RETRIES_MODEL,
    CODE_RETRIES_SANDBOX,
    CONFIRM_WORD,
    GENERATION_MODEL,
    OLLAMA_URL,
)
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
        "rag_context"
    )

    def __init__(self, task: str):
        self.state = SessionState.GENERATING_PLAN
        self.user_task = task
        self.context = {"wf": {"vars": {}, "initVariables": {}}}
        self.plan = ""
        self.plan_revision_count = 0
        self.current_code = ""
        self.code_revision_count = 0
        self.sandbox_feedback = ""
        self.created_at = time.time()
        self.rag_context = ""


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
    llm_validation: bool = True  # включить Ollama-критик после генерации кода


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
def _context_log_payload(context, max_chars: int = 4000) -> dict:
    """
    Best-effort context logger payload.

    Goal: make it obvious in logs which variables were provided, without
    accidentally dumping huge payloads or failing request handling.
    """
    try:
        if context is None:
            return {"ctx_present": False}

        wf = None
        if isinstance(context, dict) and isinstance(context.get("wf"), dict):
            wf = context.get("wf")

        wf_vars_keys = None
        wf_init_keys = None
        wf_vars_summary = None

        def _summarize_value(value, max_keys: int = 20):
            if isinstance(value, dict):
                keys = list(value.keys())
                return {"type": "object", "keys_total": len(keys), "keys": keys[:max_keys]}
            if isinstance(value, list):
                return {"type": "array", "len": len(value)}
            if isinstance(value, str):
                return {"type": "string", "len": len(value)}
            if value is None:
                return {"type": "null"}
            return {"type": type(value).__name__}

        if wf is not None:
            vars_obj = wf.get("vars")
            init_obj = wf.get("initVariables")
            if isinstance(vars_obj, dict):
                wf_vars_keys = list(vars_obj.keys())
                wf_vars_summary = {k: _summarize_value(vars_obj.get(k)) for k in wf_vars_keys[:50]}
            if isinstance(init_obj, dict):
                wf_init_keys = list(init_obj.keys())

        try:
            raw = json.dumps(context, ensure_ascii=False, default=str)
        except Exception:
            raw = str(context)

        preview = raw[:max_chars]
        return {
            "ctx_present": True,
            "ctx_type": type(context).__name__,
            "ctx_len": len(raw),
            "ctx_preview": preview,
            "ctx_truncated": len(raw) > len(preview),
            "wf_vars_keys": wf_vars_keys,
            "wf_init_keys": wf_init_keys,
            "wf_vars_summary": wf_vars_summary,
        }
    except Exception:
        return {"ctx_present": True, "ctx_log_error": True}


def _print_context_details(tag: str, context) -> None:
    """Extra explicit context printing (vars + shapes), for debugging visibility."""
    try:
        payload = _context_log_payload(context, max_chars=8000)
        if not payload.get("ctx_present"):
            print(f"[{tag}] ctx: <none>")
            return

        print(
            f"[{tag}] ctx: wf.vars keys={payload.get('wf_vars_keys')}, wf.initVariables keys={payload.get('wf_init_keys')}"
        )
        summary = payload.get("wf_vars_summary") or {}
        for k, v in summary.items():
            print(f"[{tag}] ctx var {k}: {v}")
    except Exception:
        return


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


async def validate_code(
    current_code,
    pipeline: GenerationPipeline,
    count_of_retries,
    plan,
    user_task,
    context,
):
    """Validate code in sandbox with retry loop."""
    validate_log = {
        "retries": count_of_retries,
        "plan": len(plan or ""),
        "task": len(user_task or ""),
        "code": len(current_code or ""),
    }
    validate_log.update(_context_log_payload(context))
    print(
        "[validate] start",
        validate_log,
    )
    _print_context_details("validate", context)
    sandbox_resp = await send_code_for_validation(current_code, context)
    sandbox_feedback = extract_validation_feedback(sandbox_resp)
    print(
        "[validate] initial",
        {
            "ok": sandbox_feedback is True,
            "fb_len": 0 if sandbox_feedback is True else len(str(sandbox_feedback)),
        },
    )
    for attempt in range(1, count_of_retries + 1):
        print(
            "[validate] loop",
            {
                "try": attempt,
                "ok": sandbox_feedback is True,
            },
        )
        if sandbox_feedback is not True:
            print(
                "[validate] regen",
                {
                    "try": attempt,
                    "fb_len": len(str(sandbox_feedback)),
                    "code": len(current_code or ""),
                },
            )
            fixed_code = await pipeline._generate_code(
                plan,
                user_task,
                previous_code=current_code,
                critic_feedback=f"Ошибка песочницы: {sandbox_feedback}",
                context=context,
            )
            raw_fixed = _strip_code_block(fixed_code)
            current_code = raw_fixed
            print(
                "[validate] code",
                {
                    "try": attempt,
                    "len": len(raw_fixed or ""),
                },
            )
            sandbox_resp = await send_code_for_validation(raw_fixed, context)
            sandbox_feedback = extract_validation_feedback(sandbox_resp)
            print(
                "[validate] after",
                {
                    "try": attempt,
                    "ok": sandbox_feedback is True,
                    "fb_len": 0 if sandbox_feedback is True else len(str(sandbox_feedback)),
                },
            )
        else:
            print(
                "[validate] ok",
                {
                    "try": attempt,
                    "code": len(current_code or ""),
                },
            )
            return True, current_code, None
    else:
        print(
            "[validate] fail",
            {
                "fb_len": len(str(sandbox_feedback)),
                "code": len(current_code or ""),
            },
        )
        return (
            False,
            current_code,
            str(sandbox_feedback),
        )


# ---------------------------------------------------------------------------
# Core state machine
# ---------------------------------------------------------------------------
async def _handle_plan_generation(session: SessionData) -> GenerateResponse:
    """Step 1 — generate initial plan."""
    print(
        "[plan] gen",
        {
            "task": len(session.user_task or ""),
        },
    )
    plan = await pipeline._generate_plan(session.user_task, context=session.context)
    session.plan = plan
    session.state = SessionState.AWAITING_PLAN_CONFIRMATION
    print(
        "[plan] done",
        {
            "plan": len(plan or ""),
            "state": session.state,
        },
    )
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
    print(
        "[plan] rev",
        {
            "ver": session.plan_revision_count,
            "plan": len(session.plan or ""),
            "fb": len(user_feedback or ""),
        },
    )
    refined_plan = await pipeline._generate_plan(
        f"Предыдущий план:\n{session.plan}\n\nИсправления от пользователя:\n{user_feedback}\n\nОбнови план с учётом исправлений.",
        context=session.context,
    )
    session.plan = refined_plan
    session.state = SessionState.AWAITING_PLAN_CONFIRMATION
    print(
        "[plan] done",
        {
            "ver": session.plan_revision_count,
            "plan": len(refined_plan or ""),
            "state": session.state,
        },
    )
    return GenerateResponse(
        session_id="",
        state=session.state,
        plan=refined_plan,
        message=f"План обновлён (версия {session.plan_revision_count}). Подтвердите или укажите исправления.",
    )


async def _handle_code_generation(
    session: SessionData, llm_validation: bool = True
) -> GenerateResponse:
    """Step 3 — generate code from confirmed plan, run sandbox, run Ollama critic, then return to user."""
    session.state = SessionState.GENERATING_CODE
    print(
        "[code] gen",
        {
            "task": len(session.user_task or ""),
            "plan": len(session.plan or ""),
            "rag": len(session.rag_context or ""),
            "llm": llm_validation,
        },
    )
    code = await pipeline._generate_code(
        session.plan,
        session.user_task,
        rag_data=session.rag_context,
        context=session.context,
    )
    raw_code = _strip_code_block(code)
    session.current_code = raw_code
    session.code_revision_count = 0
    session.sandbox_feedback = ""
    print(
        "[code] raw",
        {
            "len": len(raw_code or ""),
        },
    )

    # --- Pass 1: Rust sandbox (interpretation errors) ---
    successfull_validation, session.current_code, sandbox_feedback = (
        await validate_code(
            session.current_code,
            pipeline,
            CODE_RETRIES_SANDBOX,
            session.plan,
            session.user_task,
            session.context,
        )
    )
    if successfull_validation is not True:
        print(
            "[code] sandbox_fail",
            {
                "fb_len": len(str(sandbox_feedback)),
                "code": len(session.current_code or ""),
            },
        )
        session.state = SessionState.AWAITING_CODE_APPROVAL
        return GenerateResponse(
            session_id="",
            state=session.state,
            code=session.current_code,
            sandbox_feedback=sandbox_feedback,
            message=f"Сгенерированный код не прошёл проверку внутреннего валидатора. Ошибка проверки кода: {sandbox_feedback}",
        )

    # --- Pass 2: Ollama critic (logic, security, performance) ---
    critic_result = ""
    if llm_validation:
        critic_result = await pipeline._critique_code(
            session.current_code,
            rag_data=session.rag_context,
            context=session.context,
        )
        print(
            "[code] critic",
            {
                "ok": critic_result.strip().upper() == CONFIRM_WORD,
                "len": len(critic_result or ""),
            },
        )
        if critic_result.strip().upper() != CONFIRM_WORD:

            session.state = SessionState.AWAITING_CODE_APPROVAL
            return GenerateResponse(
                session_id="",
                state=session.state,
                code=session.current_code,
                sandbox_feedback=sandbox_feedback,
                message=f"Сгенерированный код не прошёл проверку внутреннего валидатора. Замечания критика: {critic_result}. \n Укажите, действительно ли нужны исправления.",
            )

    session.state = SessionState.AWAITING_CODE_APPROVAL
    print(
        "[code] done",
        {
            "state": session.state,
            "code": len(session.current_code or ""),
        },
    )
    return GenerateResponse(
        session_id="",
        state=session.state,
        code=session.current_code,
        sandbox_feedback=sandbox_feedback,
        message="Код прошёл проверки. Подтвердите или укажите исправления.",
    )


async def _handle_code_revision(
    session: SessionData, user_feedback: str, llm_validation: bool = True
) -> GenerateResponse:
    """Step 4 — revise code based on user feedback (loopable), then sandbox + Ollama critic."""
    session.code_revision_count += 1
    print(
        "[code] rev",
        {
            "ver": session.code_revision_count,
            "fb": len(user_feedback or ""),
            "task": len(session.user_task or ""),
            "plan": len(session.plan or ""),
            "prev": len(session.current_code or ""),
            "rag": len(session.rag_context or ""),
            "llm": llm_validation,
        },
    )

    revised_code = await pipeline._generate_code(
        session.plan,
        session.user_task,
        rag_data=session.rag_context,
        previous_code=session.current_code,
        critic_feedback=f"Замечания пользователя:\n{user_feedback}",
        context=session.context,
    )
    raw_revised = _strip_code_block(revised_code)
    session.current_code = raw_revised
    print(
        "[code] revised",
        {
            "ver": session.code_revision_count,
            "len": len(raw_revised or ""),
        },
    )

    # --- Pass 1: Rust sandbox ---
    print(
        "[code] revalidate",
        {
            "ver": session.code_revision_count,
            "task": len(session.user_task or ""),
        },
    )
    successfull_validation, session.current_code, sandbox_feedback = (
        await validate_code(
            session.current_code,
            pipeline,
            CODE_RETRIES_SANDBOX,
            session.plan,
            session.user_task,
            session.context,
        )
    )
    if successfull_validation is not True:
        print(
            "[code] sandbox_fail",
            {
                "ver": session.code_revision_count,
                "fb_len": len(str(sandbox_feedback)),
                "code": len(session.current_code or ""),
            },
        )
        session.state = SessionState.AWAITING_CODE_APPROVAL
        return GenerateResponse(
            session_id="",
            state=session.state,
            code=session.current_code,
            sandbox_feedback=sandbox_feedback,
            message=f"Сгенерированный код не прошёл проверку внутреннего валидатора. Ошибка проверки кода: {sandbox_feedback}",
        )



    critic_result = ""
    if llm_validation:
        critic_result = await pipeline._critique_code(
            session.current_code,
            rag_data=session.rag_context,
            context=session.context,
        )
        print(
            "[code] critic",
            {
                "ver": session.code_revision_count,
                "ok": critic_result.strip().upper() == CONFIRM_WORD,
                "len": len(critic_result or ""),
            },
        )
        if critic_result.strip().upper() != CONFIRM_WORD:
                session.state = SessionState.AWAITING_CODE_APPROVAL
                return GenerateResponse(
                    session_id="",
                    state=session.state,
                    code=session.current_code,
                    sandbox_feedback=sandbox_feedback,
                    message=f"Сгенерированный код не прошёл проверку внутреннего валидатора. Замечания критика: {critic_result}. \n Укажите, действительно ли нужны исправления.",
                )

    msg = f"Код обновлён (версия {session.code_revision_count})."
    msg += " Подтвердите или укажите исправления."
    print(
        "[code] done",
        {
            "ver": session.code_revision_count,
            "state": session.state,
            "code": len(session.current_code or ""),
        },
    )

    return GenerateResponse(
        session_id="",
        state=session.state,
        code=session.current_code,
        sandbox_feedback=sandbox_feedback,
        message=msg,
    )


# ---------------------------------------------------------------------------
# Endpoint
# ---------------------------------------------------------------------------
@app.post("/generate", response_model=GenerateResponse)
async def generate(req: GenerateRequest):
    sid, session, is_new = _get_or_create_session(req)
    req_log = {
        "sid": sid,
        "is_new": is_new,
        "state": session.state,
        "task": len(req.task or ""),
        "resp": len(req.user_response or ""),
        "llm": req.llm_validation,
    }
    req_log.update(_context_log_payload(session.context))
    print(
        "[req]",
        req_log,
    )

    if (not is_new) and req.task and req.task.strip():
        try:
            clean_task, context = extract_context_and_clean_task(req.task)

            has_real_context = (
                isinstance(context, dict)
                and isinstance(context.get("wf"), dict)
                and (
                    context["wf"].get("vars") or
                    context["wf"].get("initVariables")
                )
            )

            if has_real_context:
                session.context = context
                print("[req] ctx_updated (NEW CONTEXT DETECTED)")
            else:
                print("[req] ctx_ignored (NO REAL CONTEXT)")

            if clean_task and clean_task.strip():
                session.user_task = clean_task

        except ParseError:
            print("[req] ctx_parse_failed -> keeping old context")

    list_of_good_responses = [
        "подтвердить",
        "да",
        "согласен",
        "утверждаю",
        "approve",
        "confirm",
        "yes",
        "ok",
        "хорошо",
        "принять",
        "ок",
        "78",
        "67",
        "docker",
        "борзячка"
    ]
    # ── First call: generate plan ──────────────────────────────────────
    if session.state == SessionState.GENERATING_PLAN:
        result = await _handle_plan_generation(session)
    # ── User is confirming / revising the plan ────────────────────────
    elif session.state == SessionState.AWAITING_PLAN_CONFIRMATION:
        if req.user_response.strip().lower() in list_of_good_responses:
            session.rag_context = await build_rag_context(session.plan)
            result = await _handle_code_generation(session, req.llm_validation)
        else:
            result = await _handle_plan_revision(session, req.user_response)

    # ── Code was just generated (auto sandbox pass), waiting approval ─
    elif session.state == SessionState.AWAITING_CODE_APPROVAL:
        if req.user_response.strip().lower() == "подтвердить":
            session.state = SessionState.DONE
            result = GenerateResponse(
                session_id=sid,
                state=session.state,
                code=session.current_code,
                message="Код одобрен. Генерация завершена.",
            )
        else:
            result = await _handle_code_revision(
                session, req.user_response, req.llm_validation
            )

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

    result.session_id = sid
    print(
        "[resp]",
        {
            "sid": sid,
            "state": result.state,
            "plan": result.plan is not None,
            "code": result.code is not None,
            "fb_len": 0 if not result.sandbox_feedback else len(result.sandbox_feedback),
            "msg": len(result.message or ""),
        },
    )
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
