#!/usr/bin/env bash
# Build the pinned FFmpeg into vendor/ffmpeg so output is reproducible.
# Nothing here is optional: the version is part of every rendition record.
set -euo pipefail

FFMPEG_VERSION="${FFMPEG_VERSION:-n9.0.1}"
X264_VERSION="${X264_VERSION:-31e19f92f00c7003fa115047ce50978bc98c3a0d}"
X265_VERSION="${X265_VERSION:-4.1}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="$ROOT/vendor/ffmpeg"
BUILD="$ROOT/vendor/build"
JOBS="${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)}"

for tool in git make nasm pkg-config cmake; do
  command -v "$tool" >/dev/null || { echo "missing: $tool" >&2; exit 2; }
done

mkdir -p "$BUILD" "$PREFIX"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig:${PKG_CONFIG_PATH:-}"

clone_at() { # url ref dir
  [ -d "$3" ] || git clone "$1" "$3"
  git -C "$3" fetch --tags --depth 1 origin "$2" 2>/dev/null || git -C "$3" fetch --tags
  git -C "$3" checkout --detach "$2"
}

echo "==> x264 $X264_VERSION"
clone_at https://code.videolan.org/videolan/x264.git "$X264_VERSION" "$BUILD/x264"
( cd "$BUILD/x264"
  ./configure --prefix="$PREFIX" --enable-static --enable-pic --disable-cli
  make -j"$JOBS" && make install )

echo "==> x265 $X265_VERSION"
clone_at https://bitbucket.org/multicoreware/x265_git.git "$X265_VERSION" "$BUILD/x265"
( cd "$BUILD/x265/build/linux" 2>/dev/null || cd "$BUILD/x265/build/linux"
  cmake -G "Unix Makefiles" \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DENABLE_SHARED=OFF -DENABLE_CLI=OFF -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
    ../../source
  make -j"$JOBS" && make install )

echo "==> ffmpeg $FFMPEG_VERSION"
clone_at https://github.com/FFmpeg/FFmpeg.git "$FFMPEG_VERSION" "$BUILD/ffmpeg"
( cd "$BUILD/ffmpeg"
  ./configure \
    --prefix="$PREFIX" \
    --pkg-config-flags=--static \
    --extra-cflags="-I$PREFIX/include" \
    --extra-ldflags="-L$PREFIX/lib" \
    --enable-gpl --enable-libx264 --enable-libx265 \
    --enable-static --disable-shared --enable-pic \
    --disable-programs --enable-ffmpeg --enable-ffprobe \
    --disable-doc --disable-debug \
    --disable-network --disable-protocol=http --disable-protocol=https
  make -j"$JOBS" && make install )

cat > "$PREFIX/BUILD_INFO" <<INFO
ffmpeg=$FFMPEG_VERSION
x264=$X264_VERSION
x265=$X265_VERSION
built=$(date -u +%Y-%m-%dT%H:%M:%SZ)
INFO

echo
echo "built into $PREFIX"
echo "export PKG_CONFIG_PATH=\"$PREFIX/lib/pkgconfig:\$PKG_CONFIG_PATH\""
