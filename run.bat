@echo off

echo 🚀 Starting services...
docker compose up -d >nul 2>&1

echo 🧠 Launching TUI...
docker compose run --rm llm-tui