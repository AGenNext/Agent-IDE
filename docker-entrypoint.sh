#!/bin/sh
# Starts the backend and the Theia IDE.
# If either process exits, both are stopped so the container restarts cleanly.
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

# Start backend
node /workspace/packages/agent-ide-backend/lib/server.js &
BACKEND_PID=$!
echo "[entrypoint] backend started (pid $BACKEND_PID)"

# Wait until backend is ready before starting the IDE
i=0
until wget -qO- http://localhost:${PORT:-3001}/health >/dev/null 2>&1; do
    i=$((i+1))
    [ $i -ge 30 ] && echo "[entrypoint] backend health-check timed out" && exit 1
    sleep 1
done
echo "[entrypoint] backend ready"

# Start IDE
yarn --cwd /workspace start &
IDE_PID=$!
echo "[entrypoint] IDE started (pid $IDE_PID)"

# Exit if either child exits
wait -n
EXIT_CODE=$?
echo "[entrypoint] a process exited (code $EXIT_CODE) — stopping container"
cleanup
exit $EXIT_CODE
