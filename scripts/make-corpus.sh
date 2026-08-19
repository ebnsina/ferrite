#!/usr/bin/env bash
# Synthesise the awkward-file corpus from docs/06-verification.md.
# Each must produce a working result or a clear error — never a hang.
set -euo pipefail

OUT="${1:-testdata/corpus}"
mkdir -p "$OUT"
ff() { ffmpeg -loglevel error -y "$@"; }

echo "==> $OUT"

ff -f lavfi -i "testsrc2=size=1920x1080:rate=30:duration=8" -f lavfi -i "sine=frequency=440:duration=8" \
   -c:v libx264 -preset ultrafast -g 60 -pix_fmt yuv420p -c:a aac -shortest "$OUT/clean-1080p.mp4"

# The most common real-world mess.
ff -f lavfi -i "testsrc2=size=1280x720:rate=30:duration=8" \
   -vf "setpts=N/(30+random(0)*25)/TB" -fps_mode vfr \
   -c:v libx264 -preset ultrafast "$OUT/variable-frame-rate.mp4"

# Rotated phone video.
ff -display_rotation 90 -i "$OUT/clean-1080p.mp4" -c copy "$OUT/rotated-90.mp4"

# Interlaced broadcast.
ff -f lavfi -i "testsrc2=size=720x576:rate=25:duration=8" \
   -vf "interlace" -c:v libx264 -preset ultrafast -flags +ilme+ildct "$OUT/interlaced.mp4"

# HDR10 footage.
ff -f lavfi -i "testsrc2=size=1920x1080:rate=30:duration=6" \
   -vf "format=yuv420p10le" -c:v libx265 -preset ultrafast \
   -x265-params "colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc" \
   -tag:v hvc1 "$OUT/hdr10.mp4"

# Anamorphic: non-square pixels.
ff -f lavfi -i "testsrc2=size=720x576:rate=25:duration=6" \
   -aspect 16:9 -c:v libx264 -preset ultrafast "$OUT/anamorphic.mp4"

# No audio track.
ff -f lavfi -i "testsrc2=size=640x360:rate=30:duration=6" \
   -c:v libx264 -preset ultrafast -an "$OUT/no-audio.mp4"

# Audio starting before video.
ff -itsoffset 1.5 -f lavfi -i "testsrc2=size=640x360:rate=30:duration=6" \
   -f lavfi -i "sine=frequency=440:duration=8" \
   -map 0:v -map 1:a -c:v libx264 -preset ultrafast -c:a aac "$OUT/audio-early.mp4"

# Too short to chunk.
ff -f lavfi -i "testsrc2=size=640x360:rate=30:duration=1" \
   -c:v libx264 -preset ultrafast "$OUT/one-second.mp4"

# Tiny source: every ladder rung is bigger than it.
ff -f lavfi -i "testsrc2=size=320x240:rate=30:duration=5" \
   -c:v libx264 -preset ultrafast "$OUT/tiny-240p.mp4"

# Corrupted: a valid header with the tail replaced by noise.
ff -f lavfi -i "testsrc2=size=640x360:rate=30:duration=6" \
   -c:v libx264 -preset ultrafast "$OUT/.corrupt-src.mp4"
head -c 40000 "$OUT/.corrupt-src.mp4" > "$OUT/corrupted.mp4"
head -c 20000 /dev/urandom >> "$OUT/corrupted.mp4"
rm -f "$OUT/.corrupt-src.mp4"

ls -la "$OUT" | tail -n +2
