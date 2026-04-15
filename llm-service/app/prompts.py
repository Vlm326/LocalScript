from config import CONFIRM_WORD
import json

# нил, lowcode

# Роль 1: Архитектор (Планировщик)
SYSTEM_ARCHITECT = """
Рoль: Ты — эксперт-архитектор систем автоматизации на базе Lua 5.5.
Твоя задача — составить пошаговый план реализации алгоритма для модели-кодера.


В ответе напиши только структурирвованный план по шагам. 
Раздели логические шаги плана при помощи символа $. Пример:
$: [Описание первого шага]
$: [Описание второго шага]

Код писать НЕ надо. В ответе необходимо описать только логику работы программы
...

""".strip()

# Роль 2: Кодер (Программист)
SYSTEM_CODER = """
Роль: Ты — специализированный разработчик на Lua 5.5, работающий в среде LowCode. Твоя задача — составить код на lua по полученному плану.

Главные правила среды LowCode:
Доступ к переменным: Запрещенно обращаться к переменным с помощью JsonPath. 
Вместо этого используй только прямое обращение к данным
Для работы с массивами используй циклы: for _, item in ipairs(array) do ... end
Для вложенных структур используй цепочку обращений

Все объявленыне переменыне храни в wf.vars
Если в пользовательском вводе есть ключ "initVariables", то такие данные нужно хранить по пути wf.initVariables.имя_переменной


Используй базовый типы данных:
nil — используется для обозначения отсутствия значения.
boolean — значения true и false.
number — числа (целые и с плавающей запятой).
string — строки
array - массивы. Для работы с массивами (создание или приведение типов) используются методы:
_utils.array.new() - для создания нового массива
_utils.array.markAsArray(arr) - для объявления существующей переменной
массивом
table — таблицы, которые в Lua используются для создания массивов, списков,
ассоциативных массивов и объектов.
function — функции


Используй базовые конструкции, такие как:
if...then...else
while...do...end
for...do...end
repeat...until



В качестве предоставь исключительно готовый код, без комментариев. НЕ нужно оборачивать код в какие-либо обертки, ответ - только код.
 """.strip()
# Роль 2: Кодер (Программист) (Исправления кода)


# Роль 3: Критик (Валидатор)
# Должен проверить lowcode, а также проверить тесты (инфу) из рага. Если че то не так, отправляем фидбэк
# 
SYSTEM_CRITIC = f"""
Роль: Ты — эксперт по качеству (QA Automation) и логический аналитик систем на Lua 5.5. 
Твоя задача — проверить, выполняются ли правила LowCode. Правила LowCode:

Доступ к данным: Запрещенно обращаться к переменным с помощью JsonPath. 
Вместо этого нужно использовать только прямое обращение к данным

Все объявленыне переменыне необходимо хранить в wf.vars."имя_переменная"
Если в пользовательском вводе есть ключ "initVariables", то такие данные нужно хранить по пути wf.initVariables."имя_переменной"
a
Проверяй, не используется ли оператор # для массивов с nil-дырами, и требуй безопасного удаления элементов через table.remove вместо обнуления индексов.

Используй wf.initVariables исключительно для чтения входящих параметров, 
а любую запись или модификацию данных осуществляй только в объекте wf.vars.


Если код правильный, в качестве вывода выведи только {CONFIRM_WORD}
Если есть ошибки, в качестве ответа выведи то, что нужно исправить. Также укажи, что именно реализовано неправильно. 
""".strip()


# ─── Вспомогательные функции для сборки messages ───────────────────────────


def build_architect_messages(task: str, context: dict | None = None) -> list:
    """Сформировать messages для этапа планирования."""
    messages = [{"role": "system", "content": SYSTEM_ARCHITECT}]
    if context is not None:
        messages.append({"role": "system", "content": _format_context_block(context)})
    messages.append({"role": "user", "content": task})
    return messages


def build_coder_messages(
    plan: str,
    task: str,
    rag_data: str = "",
    previous_code: str = "",
    critic_feedback: str = "",
    context: dict | None = None,
) -> list:
    """Сформировать messages для этапа генерации / исправления кода."""
    messages = [
        {"role": "system", "content": SYSTEM_CODER},
    ]

    if context is not None:
        messages.append(
            {
                "role": "system",
                "content": _format_context_block(context),
            }
        )

    if rag_data:
        messages.append({"role": "system", "content": f"Reference data:\n{rag_data}"})

    if previous_code and critic_feedback:
        # Итерация исправления: показываем предыдущий код и фидбэк критика
        messages.append(
            {
                "role": "user",
                "content": (
                    f"Задание: {task}\n\n"
                    f"План: {plan}\n\n"
                    f"Предыдущий код:\n{previous_code}\n\n"
                    f"Critic feedback:\n{critic_feedback}\n\n"
                    f"Исправь все прообелемы и верни рабочий код.\n"
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


def _format_context_block(context: dict, max_chars: int = 4000) -> str:
    """
    Форматирует контекст для LLM как короткую, полезную подсказку.
    Важно: не раздувать промпт — контекст может быть большим.
    """
    try:
        wf = context.get("wf") if isinstance(context.get("wf"), dict) else {}
        vars_obj = wf.get("vars") if isinstance(wf.get("vars"), dict) else {}
        init_obj = (
            wf.get("initVariables")
            if isinstance(wf.get("initVariables"), dict)
            else {}
        )

        vars_keys = list(vars_obj.keys())[:100]
        init_keys = list(init_obj.keys())[:100]

        raw = json.dumps(context, ensure_ascii=False, default=str)
        preview = raw[:max_chars]
        truncated = len(raw) > len(preview)

        return (
            "Workflow context is available as global table `wf`.\n"
            f"wf.vars keys: {vars_keys}\n"
            f"wf.initVariables keys: {init_keys}\n"
            "Context JSON (truncated):\n"
            f"{preview}\n"
            f"(truncated={truncated})"
        )
    except Exception:
        return "Workflow context is available as global table `wf`."


def build_critic_messages(
        code: str,
        rag_data: str = "",
        context: dict | None = None,
                          ) -> list:
    """Сформировать messages для валидации кода."""
    messages = [
        {"role": "system", "content": SYSTEM_CRITIC},
    ]
    if context is not None:
        messages.append({"role": "system", "content": _format_context_block(context)})
    if rag_data:
        messages.append({"role": "system", "content": f"Reference data:\n{rag_data}"})
    messages.append({"role": "user", "content": f"Review this Lua code:\n\n{code}"})
    return messages
