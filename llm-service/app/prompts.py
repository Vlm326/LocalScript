from config import CONFIRM_WORD

# нил, lowcode

# Роль 1: Архитектор (Планировщик)
SYSTEM_ARCHITECT = """
Рoль: Ты — эксперт-архитектор систем автоматизации на базе Lua 5.5.
Твоя задача — составить пошаговый план реализации алгоритма для модели-кодера.


Раздели шаги плана при помощи символа $. Пример:
$: [Описание первого шага]
$: [Описание второго шага]
...

""".strip()

# Роль 2: Кодер (Программист)
SYSTEM_CODER = """
Роль: Ты — специализированный разработчик на Lua 5.5, работающий в среде LowCode. Твоя задача — составить код на lua по полученному плану.

Главные правила среды LowCode:
Доступ к переменным: Запрещенно обращаться к переменным с помощью JsonPath. 
Вместо этого используй только прямое обращение к данным

Все объявленыне переменыне храни в wf.vars
Если в пользовательском вводе есть ключ "initVariables", то такие данные нужно хранить по пути wf.initVariables.имя_переменной


Используй базовый типы данных:
● nil — используется для обозначения отсутствия значения.
● boolean — значения true и false.
● number — числа (целые и с плавающей запятой).
● string — строки
● array - массивы. Для работы с массивами (создание или приведение типов) используются методы:
_utils.array.new() - для создания нового массива
_utils.array.markAsArray(arr) - для объявления существующей переменной
массивом
● table — таблицы, которые в Lua используются для создания массивов, списков,
ассоциативных массивов и объектов.
● function — функции


Используй базовые конструкции, такие как:
● if...then...else
● while...do...end
● for...do...end
● repeat...until



Обертка: Весь код должен быть заключен в формат JsonString lua{ -- код }lua.
На выходе предоставь только готовый блок JsonString.""".strip()
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


def build_critic_messages(
        code: str,
        rag_data: str = "",
                          ) -> list:
    """Сформировать messages для валидации кода."""
    messages = [
        {"role": "system", "content": SYSTEM_CRITIC},
    ]
    if rag_data:
        messages.append({"role": "system", "content": f"Reference data:\n{rag_data}"})
    messages.append({"role": "user", "content": f"Review this Lua code:\n\n{code}"})
    return messages
