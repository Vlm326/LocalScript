import asyncio
import os
import httpx
import json

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from config import SANDBOX_SERVICE_URL

app = FastAPI(title="LLM Service", version="0.1.0")

SANDBOX_URL = os.getenv(SANDBOX_SERVICE_URL, "http://0.0.0.0:6778")


class ValidateRequest(BaseModel):
    code: str
    execute: bool = False
    timeout: int = 2

@app.post("/validate")
async def validate(req: ValidateRequest):
    async with httpx.AsyncClient(timeout=req.timeout + 5) as client:
        resp = await client.post(
            f"{SANDBOX_URL}/pipeline",
            json={"code": req.code, "execute": req.execute, "timeout": req.timeout},
        )
        if not resp.is_success:
            raise HTTPException(status_code=resp.status_code, detail=resp.text)
        return resp.json()


@app.get("/health")
async def health():
    return {"status": "ok"}

async def send_code_for_validation(code: str):
    async with httpx.AsyncClient() as client:
        resp = await client.post(
            f"{SANDBOX_URL}/pipeline",
            json={"code": code, "execute": True, "timeout": 2}
        )
        resp.raise_for_status()
        return resp.json()


def extract_validation_feedback(responce):
    # responce_dict = json.loads(responce)
    if responce_dict['status']['status'] == 'ok':
        return True
    else:
        error = responce_dict['error_detail']
        feedback = f'{error["kind"]}: "{error["message"]}" in code part: {error["snippet"]}'
        return feedback




