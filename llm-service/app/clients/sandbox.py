import httpx
from typing import Optional

from schemas import SandboxResponse


class RustSandboxClient:
    def __init__(self, base_url: str = "http://127.0.0.1:8080"):
        self.base_url = base_url.rstrip("/")

    async def validate(
        self, code: str, execute: bool = False, timeout: int = 2
    ) -> Optional[SandboxResponse]:
        try:
            async with httpx.AsyncClient(timeout=timeout + 5) as client:
                resp = await client.post(
                    f"{self.base_url}/pipeline",
                    json={"code": code, "execute": execute, "timeout": timeout},
                )
                resp.raise_for_status()
                data = resp.json()
                return SandboxResponse(**data)
        except Exception as e:
            print(f"Sandbox client error: {e}")
            return None
