#!/usr/bin/env bash
# Bring the DASH-IF conformance validator up, serving an asset directory.
set -euo pipefail

ASSET="${1:-asset}"
[ -d "$ASSET" ] || { echo "no such directory: $ASSET" >&2; exit 2; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export FERRITE_ASSET_DIR="$(cd "$ASSET" && pwd)"

docker compose -f "$ROOT/deploy/dashif/compose.yaml" up -d --build
echo "conformance at http://localhost:${FERRITE_DASHIF_PORT:-8088}, serving $FERRITE_ASSET_DIR"
