from pipeline import GenerationPipeline

pipe = GenerationPipeline("qwen2.5-coder:7b", max_retries=2)
print("Запрос к Ollama", '\n')
user_prompt = "Write a function that returns the sum of two numbers in Lua"
result = pipe.run(user_prompt, "\n")

print("Plan:", result["plan"], "\n\n\n")
print("Code:", "\n", result["raw_code"], "\n")
print(result["iterations"])
print("Feedback:", result["feedback"])

