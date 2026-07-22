# Ferrite

Multi-tenant video transcoding platform. Rust API + FFmpeg workers, SvelteKit dashboard, S3-compatible storage.

## Stack

- **API** — Rust, [axum](https://github.com/tokio-rs/axum), sqlx + PostgreSQL
- **Worker** — Rust + FFmpeg (CPU now, GPU-ready via an `Encoder` trait)
- **Queue** — Redis Streams (durable, at-least-once, dead-letter)
- **Storage** — S3-compatible (MinIO in dev)
- **Web** — SvelteKit (Svelte 5) + Tailwind v4, custom design system

## Layout

```
crates/
  core/      domain types + the Encoder trait
  storage/   S3-compatible object storage
  queue/     Redis Streams job queue
  api/        axum HTTP service
  worker/     FFmpeg transcode worker
migrations/  Postgres schema (sqlx)
web/          SvelteKit dashboard
```

## Prerequisites

Rust, Node + pnpm, Docker, and `ffmpeg` / `ffprobe` on PATH.

## Quickstart

```sh
# 1. Start Postgres, Redis, MinIO
docker compose up -d

# 2. Configure env (defaults already match the compose stack)
cp .env.example .env

# 3. Apply the database schema
cargo install sqlx-cli --no-default-features --features postgres  # once
sqlx migrate run --database-url postgres://ferrite:ferrite@localhost:5432/ferrite

# 4. Run the API and a worker (separate terminals)
cargo run -p ferrite-api
cargo run -p ferrite-worker

# 5. Run the dashboard
cd web && pnpm install && pnpm dev
```

- API health: `http://localhost:8080/health` · readiness: `/v1/ready`
- Dashboard: `http://localhost:5173`
- MinIO console: `http://localhost:9001` (ferrite / ferrite-secret)

See `make help` for shortcuts.
