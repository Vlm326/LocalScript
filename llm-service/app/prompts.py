from config import confirm_word

# Роль 1: Архитектор (Планировщик)
SYSTEM_ARCHITECT = """
You are an expert Systems Architect.
Your goal is to break down a user's request into a logical, step-by-step implementation plan.
Instructions:
1. Focus on Lua and game logic.
2. Output a numbered list of steps.
3. Be concise. Do not write any code yet.
4. If the request is unclear, make reasonable assumptions for a game environment.
Answer in Russian.
""".strip()

# Роль 2: Кодер (Программист)
SYSTEM_CODER = """
You are a Senior Lua Developer.
Your goal is to implement the provided plan into clean, efficient, and working Lua code.
Rules:
1. Write ONLY the code.
2. NO explanations, NO introductory text ("Sure, here is your code..."), NO closing remarks.
3. Use clear variable names.
4. Include brief comments inside the code only if necessary for complex logic.
5. Wrap the code in ```lua blocks.
""".strip()

# Роль 3: Критик (Валидатор)
SYSTEM_CRITIC = f"""
You are a Senior QA Engineer and Security Auditor.
Review the provided Lua code for:
1. Syntax errors or logical flaws.
2. Performance bottlenecks.
3. Potential security vulnerabilities.
Instructions:
- If the code is perfect, respond with exactly: {confirm_word}
- If there are issues, list each issue on a new line and suggest a concrete fix.
- Do NOT rewrite the entire code; only describe what needs to be changed.
Answer in Russian.
""".strip()


# ─── Вспомогательные функции для сборки messages ───────────────────────────

def build_architect_messages(task: str) -> list:
    """Сформировать messages для этапа планирования."""
    return [
        {"role": "system", "content": SYSTEM_ARCHITECT},
        {"role": "user", "content": task},
    ]


def build_coder_messages(
    plan: str,
    task: str,
    rag_data: str = "",
    previous_code: str = "",
    critic_feedback: str = "",
) -> list:
    """Сформировать messages для этапа генерации / исправления кода."""
    messages = [
        {"role": "system", "content": SYSTEM_CODER},
    ]

    if rag_data:
        messages.append(
            {"role": "system", "content": f"Reference data:\n{rag_data}"}
        )

    if previous_code and critic_feedback:
        # Итерация исправления: показываем предыдущий код и фидбэк критика
        messages.append(
            {
                "role": "user",
                "content": (
                    f"Task: {task}\n\n"
                    f"Plan: {plan}\n\n"
                    f"Previous code:\n{previous_code}\n\n"
                    f"Critic feedback:\n{critic_feedback}\n\n"
                    f"Fix all issues and return the corrected Lua code."
                ),
            }
        )
    else:
        # Первая генерация
        messages.append(
            {
                "role": "user",
                "content": f"Task: {task}\n\nPlan:\n{plan}\n\nGenerate the Lua code.",
            }
        )

    return messages


def build_critic_messages(code: str) -> list:
    """Сформировать messages для валидации кода."""
    return [
        {"role": "system", "content": SYSTEM_CRITIC},
        {"role": "user", "content": f"Review this Lua code:\n\n{code}"},
    ]

