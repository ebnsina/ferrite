#!/usr/bin/env bash
# Consistent fresh start for Ferrite's dev stack.
#
# Kills ONLY Ferrite's own processes/ports (API 8787, web 5173) and never
# touches unrelated projects (e.g. a Go service on 8091 or another Vite on 3000).
# Then starts the API, one worker, and the web dev server, each on a fixed port,
# logging to /tmp/ferrite-*.log.
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API_PORT=8787
WEB_PORT=5173

say() { printf '\033[36m›\033[0m %s\n' "$*"; }

kill_port() { # kill whatever listens on $1 (used only for Ferrite's own ports)
	local pids
	pids=$(lsof -tiTCP:"$1" -sTCP:LISTEN 2>/dev/null || true)
	[ -n "$pids" ] && kill $pids 2>/dev/null || true
}

say "Stopping Ferrite processes (leaving other projects alone)…"
pkill -f 'target/debug/ferrite-api' 2>/dev/null || true
pkill -f 'target/debug/ferrite-worker' 2>/dev/null || true
pkill -f 'Sites/ferrite/web/node_modules' 2>/dev/null || true   # this project's Vite only
kill_port "$API_PORT"
kill_port "$WEB_PORT"
sleep 1

# Infra must be up (no-op if already running).
say "Ensuring infra (Postgres/Redis/MinIO/SRS) is up…"
( cd "$ROOT" && docker compose up -d >/dev/null 2>&1 ) || true

say "Starting API on :$API_PORT …"
( cd "$ROOT" && cargo run -q -p ferrite-api >/tmp/ferrite-api.log 2>&1 & )
say "Starting worker …"
( cd "$ROOT" && cargo run -q -p ferrite-worker >/tmp/ferrite-worker.log 2>&1 & )
say "Starting web on :$WEB_PORT …"
( cd "$ROOT/web" && pnpm dev >/tmp/ferrite-web.log 2>&1 & )

# Wait for the API to answer on IPv4 (avoids any IPv6/localhost ambiguity).
for _ in $(seq 60); do
	code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$API_PORT/health" 2>/dev/null || true)
	[ "$code" = "200" ] && break
	sleep 0.5
done
for _ in $(seq 40); do
	curl -s -o /dev/null "http://127.0.0.1:$WEB_PORT/" 2>/dev/null && break
	sleep 0.5
done

echo
say "Ferrite is up:"
echo "   API   → http://localhost:$API_PORT/health  ($(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:$API_PORT/health 2>/dev/null))"
echo "   Web   → http://localhost:$WEB_PORT/         (dashboard: /app)"
echo "   Logs  → /tmp/ferrite-{api,worker,web}.log"
