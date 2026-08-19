#!/usr/bin/env bash
# Fetch the pinned Shaka Packager into vendor/. Correct HLS+DASH is a
# multi-year job with endless device quirks; we do not write one.
set -euo pipefail

VERSION="${PACKAGER_VERSION:-v3.9.3}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$ROOT/vendor/packager"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)  ASSET="packager-osx-arm64" ;;
  Darwin-x86_64) ASSET="packager-osx-x64" ;;
  Linux-aarch64) ASSET="packager-linux-arm64" ;;
  Linux-x86_64)  ASSET="packager-linux-x64" ;;
  *) echo "no packager build for $(uname -s)-$(uname -m)" >&2; exit 2 ;;
esac

URL="https://github.com/shaka-project/shaka-packager/releases/download/$VERSION/$ASSET"
mkdir -p "$(dirname "$DEST")"

if [ -x "$DEST" ] && "$DEST" --version 2>&1 | grep -q "${VERSION#v}"; then
  echo "already at $VERSION"
  exit 0
fi

echo "==> $ASSET $VERSION"
curl -fsSL "$URL" -o "$DEST.tmp"
chmod +x "$DEST.tmp"

# Record what we got. A pinned tag can be re-cut; the digest cannot.
DIGEST="$(shasum -a 256 "$DEST.tmp" | cut -d' ' -f1)"
EXPECTED_FILE="$ROOT/scripts/packager.sha256"
if [ -f "$EXPECTED_FILE" ]; then
  if ! grep -q "$ASSET $DIGEST" "$EXPECTED_FILE"; then
    echo "digest mismatch for $ASSET" >&2
    echo "  got      $DIGEST" >&2
    echo "  expected $(grep "^$ASSET " "$EXPECTED_FILE" | cut -d' ' -f2)" >&2
    rm -f "$DEST.tmp"
    exit 1
  fi
else
  echo "$ASSET $DIGEST" > "$EXPECTED_FILE"
  echo "recorded digest in scripts/packager.sha256 — commit it"
fi

mv "$DEST.tmp" "$DEST"
"$DEST" --version
echo
echo "packager at $DEST"
