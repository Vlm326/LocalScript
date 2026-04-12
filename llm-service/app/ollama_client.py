import httpx
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

    async def send_request(
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
            "stream": False,
            "keep_alive": f"{keep_alive}s",
            "options": {
                "num_ctx": self.num_ctx,
                "temperature": self.temperature,
            },
        }
        if num_predict is not None:
            payload["options"]["num_predict"] = num_predict

        try:
            timeout = httpx.Timeout(connect=10.0, read=120.0, write=10.0, pool=10.0)
            async with httpx.AsyncClient(timeout=timeout) as client:
                response = await client.post(self.api_url, json=payload)
                response.raise_for_status()
                result = response.json()
                return result.get("message", {}).get("content", "")
        except httpx.TimeoutException:
            print(f"Timeout при запросе к Ollama (model={self.model})")
            return None
        except httpx.HTTPStatusError as e:
            print(f"HTTP error {e.response.status_code} от Ollama: {e.response.text}")
            return None
        except httpx.RequestError as e:
            print(f"Ошибка запроса к Ollama: {e}")
            return None
        except (KeyError, ValueError) as e:
            print(f"Ошибка парсинга ответа Ollama: {e}")
            return None
