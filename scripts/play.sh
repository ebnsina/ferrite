#!/usr/bin/env bash
# Serve a published asset with hls.js and dash.js side by side.
# Conformance tools prove the files are correct; this proves they play.
set -euo pipefail

ASSET="${1:-asset}"
PORT="${FERRITE_PLAYER_PORT:-8099}"
[ -d "$ASSET/cmaf" ] || { echo "no $ASSET/cmaf — run 'ferrite run' first" >&2; exit 2; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cp "$ROOT/deploy/player/index.html" "$ASSET/index.html"

echo "http://localhost:$PORT/  (ctrl-c to stop)"
cd "$ASSET" && exec python3 -m http.server "$PORT"
