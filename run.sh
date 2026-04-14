#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SANDBOX_DIR="$SCRIPT_DIR/sandbox-service"
LLM_DIR="$SCRIPT_DIR/llm-service"

for name in sandbox-service llm-service; do
    if docker ps -a --format '{{.Names}}' | grep -q "^${name}$"; then
        echo "[stop] stopping and removing container: $name"
        docker stop "$name" 2>/dev/null || true
        docker rm "$name" 2>/dev/null || true
    fi
done

for img in sandbox-service llm-service; do
    if docker images --format '{{.Repository}}' | grep -q "^${img}$"; then
        echo "[clean] removing old image: $img"
        docker rmi "$img" 2>/dev/null || true
    fi
done

echo ""
echo "============================================"
echo "  Building sandbox-service (Rust)"
echo "============================================"
docker build -t sandbox-service "$SANDBOX_DIR"

echo ""
echo "============================================"
echo "  Building llm-service (Python)"
echo "============================================"
docker build -t llm-service "$LLM_DIR"

echo ""
echo "[run] starting sandbox-service on :6778"
docker run -d \
    --name sandbox-service \
    -p 6778:6778 \
    --privileged \
    sandbox-service

echo "[run] starting llm-service on :8080"
docker run -d \
    --name llm-service \
    -p 8080:8080 \
    -e SANDBOX_SERVICE_URL=http://host.docker.internal:6778 \
    llm-service

echo ""
echo "[wait] waiting for services to start..."
sleep 3

echo ""
echo "============================================"
echo "  Service status"
echo "============================================"
docker ps \
    --filter name=sandbox-service \
    --filter name=llm-service \
    --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

echo ""
echo "sandbox-service: http://localhost:6778/pipeline"
echo "llm-service:     http://localhost:8080/generate"
echo "llm-service:     http://localhost:8080/health"
echo ""
echo "To stop:  docker stop sandbox-service llm-service"
echo "To logs:  docker logs -f sandbox-service"
echo "          docker logs -f llm-service"
