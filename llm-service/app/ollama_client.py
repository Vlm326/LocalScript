import requests
from typing import Dict, List, Optional


class OllamaClient:
    def __init__(
        self,
        model_name: str,
        url: str = '127.0.0.1:11434',
        num_ctx: int = 4096,
        temperature: float = 0.0,
    ):
        self.model = model_name
        self.base_url = f"http://{url}"
        self.api_url = f"{self.base_url}/api/chat"
        self.num_ctx = num_ctx
        self.temperature = temperature

    def send_request(
        self,
        messages: List[Dict[str, str]],
        keep_alive: int = 300,
        num_predict: int = 256,
    ) -> Optional[str]:
        """Отправить список messages в Ollama и вернуть ответ.

        Args:
            messages: Список сообщений в формате [{"role": "system"|"user"|"assistant", "content": "..."}].
            keep_alive: Время удержания модели в памяти в секундах (0 — выгрузить сразу).
            num_predict: Максимальное количество генерируемых токенов.
        """
        payload: Dict = {
            "model": self.model,
            "messages": messages,
            "stream": False, # передать все сообщение сразу
            "keep_alive": f"{keep_alive}s",
            "options": {
                "num_ctx": self.num_ctx,
                "temperature": self.temperature,
            },
        }
        if num_predict is not None:
            payload["options"]["num_predict"] = num_predict

        try:
            response = requests.post(self.api_url, json=payload, timeout=120)
            response.raise_for_status()
            result = response.json()
            return result.get("message", {}).get("content", "")
        except requests.exceptions.RequestException as e:
            print(f"Ошибка при запросе к Ollama: {e}")
            return None
