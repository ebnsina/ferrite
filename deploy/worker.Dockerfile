# Ferrite worker — build from the workspace root: docker build -f deploy/worker.Dockerfile .
FROM rust:1-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --release -p ferrite-worker

FROM debian:bookworm-slim
# FFmpeg is required for transcoding + thumbnails.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates ffmpeg \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/ferrite-worker /usr/local/bin/ferrite-worker
CMD ["ferrite-worker"]
