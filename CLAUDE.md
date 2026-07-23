# Ferrite — contributor & agent guide

Self-hosted, multi-tenant video platform (VOD + live). Rust API + FFmpeg workers, SvelteKit dashboard, Redis-backed fair queue, S3-compatible storage. Covers adaptive HLS/DASH, clip/thumbnails, MP4/audio/watermark, auto-captions, AI shorts, embed + analytics, and live with simulcast + instant clipping. See `README.md` for the full feature list and API surface.

## Comments

Keep them to **1–2 lines**. Explain **why**, not what the code already says. No essays, no restating the signature. Delete a comment before writing a paragraph.

```rust
// Good — one line, explains the non-obvious reason:
// Response timeout must exceed the block window, or a valid blocking claim is killed.
let cfg = ConnectionManagerConfig::new().set_response_timeout(Some(block + Duration::from_secs(5)));
```

```rust
// Bad — essay restating the obvious:
// This function connects to Redis. It first opens a client, then it builds a
// connection manager config with a connection timeout and a response timeout,
// and the response timeout is important because ... (10 more lines)
```

Module headers: one line. A tricky invariant gets one line at the site, not a lecture.

## Conventions

- **Modular**: small focused files; one responsibility each.
- **Errors**: handle 404/500/validation from the start; never `unwrap()` on a request path. API errors go through `ApiError` → JSON envelope.
- **Docs before coding**: verify library APIs via context7; check versions against the crates.io sparse index. Don't assume.
- **Icons**: use `phosphor-svelte` icons via the `Icon` component, not inline SVG.
- **Tenancy**: every tenant-owned query is scoped by `tenant_id`.
- **Queue fairness**: enqueue is per-tenant; the scheduler round-robins with an in-flight cap. Don't bypass it with direct stream writes.
- **Config**: `FERRITE_*`, fail-fast (no serde defaults) except genuinely optional integrations (SMTP, whisper/AI, watermark). Unset optionals must degrade gracefully — log/skip/fallback, never crash.
- **Motion**: route animations through `$lib/motion` (`dur()`), so reduced-motion is one switch. Marketing scroll-reveal uses the `reveal` action.

## Layout

`crates/{core,storage,queue,api,worker}` · `web/` (SvelteKit; `app/` = dashboard, `(marketing)/` = public, `embed/` = public player) · `migrations/` (sqlx, sequential) · `deploy/` (Dockerfiles, prod compose, SRS config) · `docs/` (gitignored, internal).

Worker pipeline branches on job kind: transcode (`cmaf` + `extras` + `thumbnails` + `captions`), `clip`, and `shorts`. New migrations are additive and numbered.

## Dev

`docker compose up -d` · `sqlx migrate run` · `cargo run -p ferrite-api` · `cargo run -p ferrite-worker` · `cd web && pnpm dev`. Frontend on `:5173` (`/` marketing, `/app` dashboard). See `Makefile`.

## Git

Commit per feature. Author `ebnsina <ebnsina.me@gmail.com>`. Never commit `docs/`, `data/`, or `.env`.
