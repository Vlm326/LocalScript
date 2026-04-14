import json
import os
from typing import Any, Optional

import httpx

SANDBOX_URL = os.getenv("SANDBOX_SERVICE_URL", "http://0.0.0.0:6778")


async def send_code_for_validation(
    code: str,
    context: Optional[dict[str, Any]] = None,
    execute: bool = True,
    timeout: int = 2,
):
    async with httpx.AsyncClient(timeout=timeout + 5) as client:
        payload = {
            "code": code,
            "execute": execute,
            "timeout": timeout,
            "context": context,
        }
        resp = await client.post(f"{SANDBOX_URL}/pipeline", json=payload)
        resp.raise_for_status()
        return resp.json()


def extract_validation_feedback(response: Any):
    if isinstance(response, str):
        response = json.loads(response)

    # if response.get("status") == "ok":
    #     return True
    status = response.get("status")
    error = response.get("error_detail")
    # if not error:
    #     return "No errors"

    snippet = error.get("snippet")
    feedback = f'{error["kind"]}: "{error["message"]}"'
    if snippet:
        feedback += f"\nin code part:\n{snippet}"
    return feedback
