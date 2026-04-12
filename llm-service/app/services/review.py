import prompts
from config import confirm_word
from clients.ollama import OllamaClient


class ReviewService:
    def __init__(self, ollama: OllamaClient):
        self.ollama = ollama

    async def review(self, code: str) -> str:
        messages = prompts.build_critic_messages(code)
        return self.ollama.send_request(messages, keep_alive=300) or ""

    def is_ok(self, feedback: str) -> bool:
        return confirm_word in feedback.upper()
