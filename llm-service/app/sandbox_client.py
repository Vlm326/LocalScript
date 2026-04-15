import json
import os
from typing import Any, Optional

import httpx
from models import SandboxResponse

SANDBOX_URL = os.getenv("SANDBOX_SERVICE_URL", "http://sandbox-service:6778")


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


def _normalize_sandbox_response(response: Any) -> dict[str, Any]:
    if isinstance(response, str):
        response = json.loads(response)

    if not isinstance(response, dict):
        raise TypeError(f"Unexpected sandbox response type: {type(response)}")

    # Частый случай: ответ завернут в sandbox_result
    if "sandbox_result" in response and isinstance(response["sandbox_result"], dict):
        response = response["sandbox_result"]

    # Частый случай: status приходит как {"status": "ok"} вместо строки
    status = response.get("status")
    if isinstance(status, dict):
        response = response.copy()
        response["status"] = (
            status.get("status")
            or status.get("value")
            or status.get("code")
        )

    return response


def extract_validation_feedback(response: Any):
    response = _normalize_sandbox_response(response)
    sandbox = SandboxResponse.model_validate(response)

    if sandbox.is_ok:
        return True

    if sandbox.error_detail:
        feedback = f'{sandbox.error_detail.kind}: "{sandbox.error_detail.message}"'
        if sandbox.error_detail.snippet:
            feedback += f"\nin code part:\n{sandbox.error_detail.snippet}"
        return feedback

    return sandbox.error_summary or "unknown sandbox error"