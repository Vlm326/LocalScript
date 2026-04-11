from ollama_client import OllamaClient
import prompts
from config import confirm_word


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
        host: str = "127.0.0.1",
        port: int = 11434,
        max_retries: int = 2,
    ):
        self.client = OllamaClient(model_name, host, port)
        self.max_retries = max_retries

    def run(self, user_input: str, rag_data: str = "") -> dict:
        """Запустить полный конвейер: Plan → Generate → Validate (loop).

        Возвращает словарь с plan, raw_code (финальный), feedback и списком
        всех итераций исправления (iterations).
        """
        plan = self._get_plan(user_input)
        raw_code = self._generate_code(plan, user_input, rag_data)

        iterations = []
        feedback = ""
        for attempt in range(1, self.max_retries + 1):
            feedback = self._critique_code(raw_code)

            if self._is_code_ok(feedback):
                # Код прошёл валидацию
                return {
                    "plan": plan,
                    "raw_code": raw_code,
                    "feedback": feedback,
                    "iterations": iterations,
                    "status": "ok",
                }

            corrected_code = self._fix_code(
                plan, user_input, rag_data, raw_code, feedback
            )
            iterations.append(
                {
                    "attempt": attempt,
                    "code_before": raw_code,
                    "feedback": feedback,
                    "code_after": corrected_code,
                }
            )
            raw_code = corrected_code

        return {
            "plan": plan,
            "raw_code": raw_code,
            "feedback": feedback,
            "iterations": iterations,
            "status": "retries_exhausted",
        }

    def _get_plan(self, task: str) -> str:
        messages = prompts.build_architect_messages(task)
        result = self.client.send_request(messages, keep_alive=300)
        return result or ""

    def _generate_code(self, plan: str, task: str, rag_data: str = "") -> str:
        messages = prompts.build_coder_messages(plan=plan, task=task, rag_data=rag_data)
        result = self.client.send_request(messages, keep_alive=300)
        return result or ""

    def _critique_code(self, code: str) -> str:
        messages = prompts.build_critic_messages(code)
        result = self.client.send_request(messages, keep_alive=300)
        return result or ""

    def _fix_code(
        self,
        plan: str,
        task: str,
        rag_data: str,
        previous_code: str,
        critic_feedback: str,
    ) -> str:
        messages = prompts.build_coder_messages(
            plan=plan,
            task=task,
            rag_data=rag_data,
            previous_code=previous_code,
            critic_feedback=critic_feedback,
        )
        result = self.client.send_request(messages, keep_alive=300)
        return result or ""

    def _is_code_ok(self, feedback: str) -> bool:
        """Проверить, что критик принял код (содержит CODE_OK)."""
        return confirm_word in feedback.upper()
