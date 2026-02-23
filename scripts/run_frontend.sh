#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTEND_DIR="$ROOT_DIR/frontend"
LOG_FILE="${LOG_FILE:-/tmp/agent_zero_frontend.log}"
PORT="${FRONTEND_PORT:-5173}"

cd "$ROOT_DIR"

echo "[frontend] stopping existing frontend process on port $PORT (if any)..."
lsof -ti:"$PORT" | xargs kill -9 2>/dev/null || true
pkill -f "npm --prefix frontend run dev" 2>/dev/null || true
pkill -f "vite" 2>/dev/null || true

echo "[frontend] building latest frontend bundle..."
npm --prefix "$FRONTEND_DIR" run build

echo "[frontend] starting dev server..."
nohup npm --prefix "$FRONTEND_DIR" run dev -- --host 127.0.0.1 --port "$PORT" --strictPort > "$LOG_FILE" 2>&1 &
PID=$!

echo "[frontend] started with PID $PID"
echo "[frontend] log: $LOG_FILE"

if ! kill -0 "$PID" 2>/dev/null; then
  echo "[frontend] failed to stay running; last log lines:" >&2
  tail -n 50 "$LOG_FILE" >&2 || true
  exit 1
fi

for _ in $(seq 1 20); do
  if curl -fsS "http://127.0.0.1:$PORT" >/dev/null; then
    echo "[frontend] health check passed on http://127.0.0.1:$PORT"
    exit 0
  fi
  sleep 1
done

echo "[frontend] process is running but health check failed; inspect $LOG_FILE" >&2
tail -n 50 "$LOG_FILE" >&2 || true
exit 1
