# Ferrite

Self-hosted, multi-tenant video platform — VOD **and** live. Rust API + FFmpeg
workers, a Redis-backed fair queue, S3-compatible storage, and a SvelteKit
dashboard. Everything runs on your own infrastructure and stores in your own
bucket.

## Features

**Encoding & packaging**
- Adaptive **HLS + DASH** from a single CMAF encode (shared fMP4 segments)
- **Per-title** (content-aware) bitrate ladders — tailored per source
- Optional **MP4 download**, **audio-only (M4A)**, and **AES-128** encrypted HLS
- **Watermark** — burn a brand logo onto the stream and the MP4 download
- Poster frames, sprite sheets, and WebVTT **storyboards**

**Editing & AI**
- **Clip / trim** a source into a new asset
- **On-demand thumbnails** at any timestamp + animated hover **previews**
- **Auto-captions** (WebVTT) — local `whisper.cpp` or any OpenAI-compatible endpoint
- **AI vertical shorts** — pick highlights (agnostic LLM or local heuristic),
  reframe to 9:16, burn in captions → new assets

**Live**
- **RTMP + SRT** ingest, low-latency HTTP-FLV playback (via SRS)
- Auto-archival of live sessions to VOD
- **Simulcast / restream** to YouTube, Twitch, etc. (RTMP fan-out)
- **Instant live clipping** — capture a moment mid-broadcast

**Delivery & embedding**
- **Signed playback** proxy (private outputs, expiring HMAC tokens, playlist rewriting)
- **Embeddable player** (`<iframe>`, rendition selector, captions) + copyable embed code
- **Playback analytics** — views, watch-time, completion rate

**Platform**
- Multi-tenant **auth** (email/password JWT + `frt_` API keys), team **members**
  & roles, invites, password reset
- **Fair queue** — per-tenant round-robin with an in-flight cap (one tenant's
  10,000 jobs can't starve another's)
- Webhooks, usage metering (mock billing), Prometheus `/metrics`

## Stack

- **API** — Rust, [axum](https://github.com/tokio-rs/axum) 0.8, sqlx + PostgreSQL
- **Worker** — Rust + FFmpeg (CPU now, GPU-ready via an `Encoder` trait)
- **Queue** — Redis Streams (durable, at-least-once, dead-letter)
- **Storage** — S3-compatible (MinIO in dev)
- **Ingest** — SRS (RTMP/SRT/HLS/FLV) for live
- **Web** — SvelteKit (Svelte 5) + Tailwind v4, custom design system

## Layout

```
crates/
  core/      domain types + the Encoder trait (TranscodeJob, Ladder, MediaInfo…)
  storage/   S3-compatible object storage
  queue/     Redis Streams fair job queue
  api/       axum HTTP service (auth, jobs, live, media, analytics, email, relay)
  worker/    FFmpeg pipeline (cmaf, extras, thumbnails, clip, captions, shorts)
migrations/  Postgres schema (sqlx)
deploy/      Dockerfiles, prod compose, SRS config
web/         SvelteKit dashboard + marketing site
```

## Prerequisites

Rust, Node + pnpm, Docker, and `ffmpeg` / `ffprobe` on PATH. For burned captions
in shorts, an FFmpeg built with `libass`.

## Quickstart

```sh
# 1. Start Postgres, Redis, MinIO (and SRS for live)
docker compose up -d

# 2. Configure env (defaults match the compose stack)
cp .env.example .env
cp web/.env.example web/.env

# 3. Apply the database schema
cargo install sqlx-cli --no-default-features --features postgres   # once
sqlx migrate run --database-url postgres://ferrite:ferrite@localhost:5432/ferrite

# 4. Run the API and a worker (separate terminals)
cargo run -p ferrite-api
cargo run -p ferrite-worker

# 5. Run the frontend
cd web && pnpm install && pnpm dev
```

- Marketing site: `http://localhost:5173/` · Dashboard: `http://localhost:5173/app`
- API health: `http://localhost:8080/health`
- MinIO console: `http://localhost:9001`

Config lives in `.env` (`FERRITE_*`, fail-fast — a missing required value stops
startup). See `.env.example` for the full list and `make help` for shortcuts.

## Optional integrations

All optional — unset means the feature degrades gracefully (logged, skipped, or
falls back), so Ferrite builds and runs without any of them.

- **Email** (invites, password reset): `FERRITE_SMTP_HOST/PORT/USER/PASSWORD/FROM`.
  Unset → emails are logged to the console.
- **Auto-captions & AI shorts**: local whisper.cpp (`FERRITE_WHISPER_BIN` +
  `FERRITE_WHISPER_MODEL`) **or** any OpenAI-compatible endpoint
  (`FERRITE_AI_BASE_URL` + `FERRITE_AI_KEY` + `FERRITE_AI_MODEL`);
  `FERRITE_AI_CHAT_MODEL` powers AI highlight selection (heuristic fallback
  without it). Nothing is provider-locked.
- **Live**: the bundled SRS service (`deploy/srs.conf`) with hooks back to the
  API (`FERRITE_LIVE_*`, `FERRITE_LIVE_HOOK_SECRET`).

## API surface (v1)

- **Auth**: `POST /v1/auth/{signup,login,forgot-password,reset-password}`
- **Profile / team**: `GET|PATCH /v1/profile`, `POST /v1/profile/password`,
  `GET|POST /v1/members`, `PATCH|DELETE /v1/members/{id}`,
  `GET|POST /v1/api-keys`, `DELETE /v1/api-keys/{id}`, `GET /v1/brand` + logo
- **Assets**: `POST /v1/assets` (presigned upload), `.../complete`,
  `.../clip`, `.../shorts`; `GET /v1/assets[/{id}]`
- **Jobs**: `POST /v1/jobs` (+ options: mp4/audio/captions/encrypt/watermark),
  `/jobs/batch`, `GET /v1/jobs[/{id}]`, `/jobs/{id}/{events,analytics,embed}`
- **Live**: `GET|POST /v1/live/streams`, `/{id}/{clip,targets}`
- **Media** (signed, public): `GET /media/{asset}/{thumbnail,preview}`
- **Playback** (signed, public): `GET /playback/{job}/{*path}`,
  `POST /playback/beacon`
- **Ops**: `GET /v1/usage`, `GET /metrics`, `GET|POST /v1/webhooks`

Programmatic clients authenticate with an `frt_` API key; the dashboard uses a
session JWT. Both resolve to the same `TenantContext`.

## Deploy

`deploy/` has multi-stage Dockerfiles for the API, worker, and web, a
`docker-compose.prod.yml` that builds and runs them against the infra, and a
GitHub Actions CI (`fmt` + `clippy` + build for Rust; `check` + build for web).

```sh
docker compose -f docker-compose.yml -f deploy/docker-compose.prod.yml up -d --build
```

## Status

Shipped: the full VOD + live + AI + delivery pipeline above. **Deferred**
(need external hardware/services): GPU/NVENC encoding, DRM, and real billing
(usage is metered but billing is mocked). Forensic (invisible/per-viewer)
watermarking is not implemented — only visible logo overlay.
