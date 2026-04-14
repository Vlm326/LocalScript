Write-Host "🚀 Starting services..."

docker compose up -d *> $null

Write-Host "🧠 Launching TUI..."

docker compose run --rm llm-tui