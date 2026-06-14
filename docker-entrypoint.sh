#!/bin/sh
# Phase 1: Node backend + Theia IDE
# Phase 2: Rust agent-runner + Theia IDE (set RUNTIME_PHASE=2)
set -e

BACKEND_PID=""
IDE_PID=""

cleanup() {
    echo "[entrypoint] shutting down..."
    [ -n "$BACKEND_PID" ] && kill "$BACKEND_PID" 2>/dev/null || true
    [ -n "$IDE_PID" ]     && kill "$IDE_PID"     2>/dev/null || true
    wait
}
trap cleanup INT TERM

# ── Start backend (phase-aware) ───────────────────────────────────────────────
if [ "${RUNTIME_PHASE:-1}" = "2" ]; then
    echo "[entrypoint] Phase 2 — Rust agent-runner"
    /usr/local/bin/agent-runner &
else
    echo "[entrypoint] Phase 1 — Node backend"
    node /workspace/packages/agent-ide-backend/lib/server.js &
fi
BACKEND_PID=$!

# ── Health gate before IDE starts ─────────────────────────────────────────────
i=0
until wget -qO- http://localhost:${PORT:-3001}/health >/dev/null 2>&1; do
    i=$((i+1))
    [ $i -ge 30 ] && echo "[entrypoint] backend health-check timed out" && exit 1
    sleep 1
done
echo "[entrypoint] backend ready (phase ${RUNTIME_PHASE:-1})"

# ── Start Theia IDE ───────────────────────────────────────────────────────────
yarn --cwd /workspace start &
IDE_PID=$!
echo "[entrypoint] IDE started (pid $IDE_PID)"

wait -n
EXIT_CODE=$?
echo "[entrypoint] process exited (code $EXIT_CODE) — stopping container"
cleanup
exit $EXIT_CODE
