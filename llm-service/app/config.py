import os

# ─── Ollama ──────────────────────────────────────────────────────────────
MODEL_NAME = os.getenv("MODEL_NAME", "qwen2.5-coder:7b")
OLLAMA_HOST = os.getenv("OLLAMA_HOST", "127.0.0.1")
OLLAMA_PORT = int(os.getenv("OLLAMA_PORT", "11434"))

# ─── Sandbox-service ────────────────────────────────────────────────────
SANDBOX_SERVICE_URL = os.getenv("SANDBOX_SERVICE_URL", "http://127.0.0.1:8080")

# ─── Pipeline ───────────────────────────────────────────────────────────
MAX_RETRIES = int(os.getenv("MAX_RETRIES", "2"))
MAX_SANDBOX_RETRIES = int(os.getenv("MAX_SANDBOX_RETRIES", "2"))

# ─── Критик ─────────────────────────────────────────────────────────────
confirm_word = "CODE_OK"
