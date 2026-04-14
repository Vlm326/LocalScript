import time

import prompts
from config import CONFIRM_WORD, CODE_RETRIES_MODEL
from ollama_client import OllamaClient


class GenerationPipeline:
    """Конвейер генерации Lua-кода с self-correction loop.

    Параметры
    ---------
    model_name : str
        Имя модели Ollama.
    host, port : str / int
        Адрес Ollama-сервера.
    max_retries : int
        Максимальное количество итераций исправления кода.
    """

    def __init__(
        self,
        model_name: str,
        url: str = "127.0.0.1:11434",
        max_retries: int = 2,
    ):
        self.client = OllamaClient(model_name, url=url)
        self.max_retries = CODE_RETRIES_MODEL

    async def _generate_plan(self, task: str) -> str:
        start_plan_time = time.perf_counter()
        messages = prompts.build_architect_messages(task)
        result = await self.client.send_request(messages, keep_alive=300)
        end_plan_time = time.perf_counter()
        print("=" * 15, "\n", "PLAN_TIME: ", end_plan_time - start_plan_time)
    
        if not result or not result.strip():
            raise RuntimeError("Ollama returned empty plan")
        return result

    async def _generate_code(
        self,
        plan: str,
        task: str,
        rag_data: str = "",
        previous_code: str = "",
        critic_feedback: str = "",
    ) -> str:
        start_code_time = time.perf_counter()
        messages = prompts.build_coder_messages(
            plan=plan,
            task=task,
            rag_data=rag_data,
            previous_code=previous_code,
            critic_feedback=critic_feedback,
        )
        result = await self.client.send_request(messages, keep_alive=300)
        end_code_time = time.perf_counter()
        print("=" * 15, "\n", "CODE_TIME: ", end_code_time - start_code_time)
        return result or ""

    async def _critique_code(self, code: str, rag_data: str = "") -> str:
        start_feedback_time = time.perf_counter()
        messages = prompts.build_critic_messages(code, rag_data=rag_data)
        result = await self.client.send_request(messages, keep_alive=300)
        end_feedback_time = time.perf_counter()
        print(
            "=" * 15, "\n", "FEEDBACK_TIME: ", end_feedback_time - start_feedback_time
        )
        return result or ""


    def _is_code_ok(self, feedback: str) -> bool:
        """Проверить, что критик принял код (содержит CODE_OK)."""
        return CONFIRM_WORD in feedback.upper()
