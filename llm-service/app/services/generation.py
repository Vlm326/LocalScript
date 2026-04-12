import prompts
from clients.ollama import OllamaClient


class GenerationService:
    def __init__(self, ollama: OllamaClient):
        self.ollama = ollama

    async def plan(self, task: str) -> str:
        messages = prompts.build_architect_messages(task)
        return self.ollama.send_request(messages, keep_alive=300) or ""

    async def generate(
        self, plan: str, task: str, rag_data: str = ""
    ) -> str:
        messages = prompts.build_coder_messages(
            plan=plan, task=task, rag_data=rag_data
        )
        return self.ollama.send_request(messages, keep_alive=300) or ""
