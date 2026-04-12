from ollama_client import OllamaClient
import prompts
from config import confirm_word
import time


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
        total_time = 0

        start_plan_time = time.perf_counter()
        plan = self._get_plan(user_input)
        end_plan_time = time.perf_counter()
        print("=" * 15, "\n", "PLAN_TIME: ", end_plan_time - start_plan_time)
        total_time += end_plan_time - start_plan_time

        start_code_time = time.perf_counter()
        raw_code = self._generate_code(plan, user_input, rag_data)
        end_code_time = time.perf_counter()
        print("=" * 15, "\n", "CODE_TIME: ", end_code_time - start_code_time)
        total_time += end_code_time - start_code_time

        iterations = []
        feedback = ""

        for attempt in range(1, self.max_retries + 1):

            start_feedback_time = time.perf_counter()
            feedback = self._critique_code(raw_code)
            end_feedback_time = time.perf_counter()
            print(
                "=" * 15,
                "\n",
                "FEEDBACK_TIME: ",
                end_feedback_time - start_feedback_time,
            )
            total_time += end_feedback_time - start_feedback_time

            if self._is_code_ok(feedback):
                # Код прошёл валидацию
                print("=" * 20, "TOTAL_TIME: ", total_time)

                return {
                    "plan": plan,
                    "raw_code": raw_code,
                    "feedback": feedback,
                    "iterations": iterations,
                    "status": "ok",
                }

            start_code_time = time.perf_counter()
            corrected_code = self._fix_code(
                plan, user_input, rag_data, raw_code, feedback
            )
            end_code_time = time.perf_counter()
            print("=" * 15, "\n", "CODE_TIME: ", end_code_time - start_code_time)
            total_time = end_code_time - start_code_time

            iterations.append(
                {
                    "attempt": attempt,
                    "code_before": raw_code,
                    "feedback": feedback,
                    "code_after": corrected_code,
                }
            )
            raw_code = corrected_code

        print("20" * 20, "TOTAL_TIME: ", total_time)

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
