from pipeline import GenerationPipeline

pipe = GenerationPipeline("qwen2.5-coder:7b", max_retries=2)
print("Запрос к Ollama", '\n')
user_prompt = '''
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
'''
result = pipe.run(user_prompt, "\n")

print("Plan:", result["plan"], "\n\n\n")
print("Code:", "\n", result["raw_code"], "\n")
print(result["iterations"])
print("Feedback:", result["feedback"])

