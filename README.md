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
- **Multi-language captions** — translate transcripts to any language (agnostic)
- **Content moderation** — classify the transcript for policy safety on ingest

**Discover, verify & experience** (what hosted platforms don't offer)
- **In-video search** — search the spoken words across your library, jump to the moment
- **Content provenance** — Ed25519-signed, tamper-evident credentials with edit lineage
- **Interactive transcript** — click a line to seek; copy a link to any moment

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
- Multi-tenant **auth** (HttpOnly-cookie sessions + `frt_` API keys), team
  **members** & roles, invites, password reset, config-driven **superadmin**
- **Two-layer tenant isolation** — every query scoped by `tenant_id`, backed by
  Postgres **row-level security** so a missing filter still can't cross tenants
- **CSRF, CORS allowlist & OWASP headers** — double-submit CSRF, credentialed
  CORS to the app origin, `nosniff` / frame-`DENY` / HSTS / referrer-policy
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
sqlx migrate run --database-url postgres://ferrite:ferrite@localhost:5455/ferrite

# 4. Run the API and a worker (separate terminals)
cargo run -p ferrite-api
cargo run -p ferrite-worker

# 5. Run the frontend
cd web && pnpm install && pnpm dev
```

- Marketing site: `http://localhost:5173/` · Dashboard: `http://localhost:5173/app`
- API health: `http://localhost:8787/health`
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

## Running fully offline / air-gapped

Every AI feature is provider-agnostic and can run **entirely on your own
hardware** — no content ever leaves your network. This is the compliance story
hosted platforms structurally can't offer (healthcare, legal, defense, EU data
residency):

- **Transcription / captions / shorts / search index** → local **whisper.cpp**
  (`FERRITE_WHISPER_BIN` + `FERRITE_WHISPER_MODEL`).
- **Translation, moderation, AI highlight selection** → point `FERRITE_AI_BASE_URL`
  at a **local OpenAI-compatible server** (e.g. [Ollama](https://ollama.com) or
  vLLM) instead of a cloud API:

  ```sh
  # .env — fully local AI via Ollama (no data egress)
  FERRITE_AI_BASE_URL=http://localhost:11434/v1
  FERRITE_AI_KEY=ollama                 # any non-empty value
  FERRITE_AI_CHAT_MODEL=llama3.1        # translation / moderation / highlights
  ```

- **Provenance signing** is local Ed25519 (`FERRITE_PROVENANCE_SECRET`).

Differentiator features vs hosted platforms: **in-video search**, **content
provenance / verifiable credentials**, **multi-language captions**, **on-ingest
moderation**, and an **interactive transcript** — all self-hosted and, when you
want, fully offline.

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

Programmatic clients authenticate with an `frt_` API key (`Authorization:
Bearer`); the dashboard uses an HttpOnly session cookie. Both resolve to the
same `TenantContext`.

## Security

- **Tenant isolation, two layers.** Every tenant query is scoped by `tenant_id`
  (the belt) *and* by Postgres **row-level security** (the suspenders): even a
  query that forgot its filter returns nothing across tenants. RLS binds only
  for a non-superuser role, so the **API connects as `ferrite_app`**
  (`FERRITE_API_DATABASE_URL`) and sets `app.current_tenant` per request; the
  **worker keeps the owner role** and does its cross-tenant work by bypass. See
  migration `0017_rls`.
- **Dashboard auth.** The session JWT lives in an **HttpOnly, SameSite=Lax**
  cookie (unreadable to JS, so XSS can't steal it), `Secure` whenever the app is
  served over HTTPS. State-changing requests carry a **double-submit CSRF**
  token (`ferrite_csrf` cookie ↔ `X-CSRF-Token` header); `Authorization: Bearer`
  (API keys) is CSRF-exempt.
- **Boundary.** `/v1` uses **credentialed CORS locked to `FERRITE_APP_BASE_URL`**;
  public embed/media/playback/waitlist routes stay permissive so third-party
  embeds work. Every response carries OWASP headers (`nosniff`, frame-`DENY`,
  HSTS, `Referrer-Policy`, CORP).
- **Superadmin** is config-driven via `FERRITE_SUPERADMIN_EMAILS` → a `superadmin`
  JWT claim gating the cross-tenant `/admin` console.

Production notes: create the `ferrite_app` role with a real password and point
`FERRITE_API_DATABASE_URL` at it (the migration seeds a dev-only default). If the
API is served from a different *site* than the dashboard (not just a subdomain),
switch the session cookie to `SameSite=None`.

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
