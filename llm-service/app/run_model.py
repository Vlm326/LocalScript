from pipeline import GenerationPipeline

pipe = GenerationPipeline("qwen2.5-coder:7b", max_retries=2)
result = pipe.run("Write a function that returns the sum of two numbers in Lua", "\n")

print("Plan:", result["plan"][:100], "\n\n\n")
print("Code:", "\n", result["raw_code"][:200], "\n")
print("Feedback:", result["feedback"][:100], "...")


