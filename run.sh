#!/usr/bin/env bash

set -e

echo "🚀 Starting services..."
docker compose up -d > /dev/null 2>&1

echo "🧠 Launching TUI..."
docker compose run --rm llm-tui