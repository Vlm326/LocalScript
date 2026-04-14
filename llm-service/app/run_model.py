import asyncio
from pipeline import GenerationPipeline
from config import CODE_RETRIES_MODEL


async def main():
    # 1. Инициализация пайплайна
    # Убедитесь, что конструктор не требует await (обычно __init__ синхронный)
    pipe = GenerationPipeline("qwen2.5-coder:7b")

    print("Запрос к Ollama", "\n")

    user_prompt = """
Для полученных данных из предыдущего REST запроса очисти значения переменных ID,ENTITY_ID, CALL
{
"wf": {
"vars": {
"RESTbody": {
"result": [
{
"ID": 123,
"ENTITY_ID": 456,
"CALL": "example_call_1",
"OTHER_KEY_1": "value1",
"OTHER_KEY_2": "value2"
},
{
"ID": 789,
"ENTITY_ID": 101,
"CALL": "example_call_2",
"EXTRA_KEY_1": "value3",
"EXTRA_KEY_2": "value4"
}
]
}
}
}
}
"""

    try:
        plan = await pipe._generate_plan(user_prompt)
        result = await pipe._generate_code(user_prompt)
        feedback

        print("Результат:")
        print(result)

    except Exception as e:
        print(f"Произошла ошибка: {e}")


if __name__ == "__main__":
    # 3. Запуск асинхронного цикла
    asyncio.run(main())
