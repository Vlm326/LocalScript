# llm-service/app/config.py
import os

GENERATION_MODEL = os.getenv("GENERATION_MODEL", "qwen2.5-coder:7b")

OLLAMA_URL = os.getenv("OLLAMA_URL", "ollama:11434")
SANDBOX_SERVICE_URL = os.getenv("SANDBOX_SERVICE_URL", "http://sandbox-service:6778")

CONFIRM_WORD = os.getenv("CONFIRM_WORD", "CODE_OK")
MAX_RETRIES = int(os.getenv("MAX_RETRIES", "2"))
CODE_RETRIES_COUNT = int(os.getenv("CODE_RETRIES_COUNT", "5"))

HOST = os.getenv("HOST", "0.0.0.0")
PORT = int(os.getenv("PORT", "8080"))