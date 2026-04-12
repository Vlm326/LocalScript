import os

import httpx
from fastapi import FastAPI, HTTPException

from models import SandboxResponse, ValidateRequest, ValidateResponse

app = FastAPI(title="LLM Service", version="0.1.0")

SANDBOX_URL = os.getenv("SANDBOX_SERVICE_URL", "http://127.0.0.1:6778")


@app.post("/validate", response_model=ValidateResponse)
async def validate(req: ValidateRequest):
    async with httpx.AsyncClient(timeout=req.timeout + 5) as client:
        payload = {
            "code": req.code,
            "execute": req.execute,
            "timeout": req.timeout,
        }
        if req.context is not None:
            payload["context"] = req.context

        resp = await client.post(
            f"{SANDBOX_URL}/pipeline",
            json=payload,
        )
        if not resp.is_success:
            raise HTTPException(status_code=resp.status_code, detail=resp.text)
        data = resp.json()
        sandbox_result = SandboxResponse(**data)
        return ValidateResponse(
            sandbox_result=sandbox_result,
            code=req.code,
        )


@app.get("/health")
async def health():
    return {"status": "ok"}
