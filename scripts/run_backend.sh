#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="$ROOT_DIR/backend"
LOG_FILE="${LOG_FILE:-/tmp/agent_zero_backend.log}"
PORT="${BACKEND_PORT:-8000}"

cd "$BACKEND_DIR"

echo "[backend] stopping existing process on port $PORT (if any)..."
lsof -ti:"$PORT" | xargs kill -9 2>/dev/null || true
pkill -f "target/release/agent_zero_backend" 2>/dev/null || true
pkill -f "cargo run --release" 2>/dev/null || true

echo "[backend] building release binary..."
cargo build --release

echo "[backend] starting fresh binary..."
nohup ./target/release/agent_zero_backend > "$LOG_FILE" 2>&1 &
PID=$!

echo "[backend] started with PID $PID"
echo "[backend] log: $LOG_FILE"

sleep 2
if ! kill -0 "$PID" 2>/dev/null; then
  echo "[backend] failed to stay running; last log lines:" >&2
  tail -n 50 "$LOG_FILE" >&2 || true
  exit 1
fi

if curl -fsS "http://127.0.0.1:$PORT/api/listings" >/dev/null; then
  echo "[backend] health check passed on http://127.0.0.1:$PORT"
else
  echo "[backend] process is running but health check failed; inspect $LOG_FILE" >&2
  exit 1
fi
