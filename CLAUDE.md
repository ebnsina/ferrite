# Ferrite — contributor & agent guide

Multi-tenant video transcoder. Rust API + FFmpeg workers, SvelteKit dashboard, Redis-backed fair queue, S3-compatible storage. VOD first, live later.

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
- **Icons**: use the icon lib (`@lucide/svelte`), not inline SVG.
- **Tenancy**: every tenant-owned query is scoped by `tenant_id`.
- **Queue fairness**: enqueue is per-tenant; the scheduler round-robins with an in-flight cap. Don't bypass it with direct stream writes.

## Layout

`crates/{core,storage,queue,api,worker}` · `web/` (SvelteKit) · `migrations/` · `docs/` (gitignored, internal).

## Dev

`docker compose up -d` · `sqlx migrate run` · `cargo run -p ferrite-api` · `cargo run -p ferrite-worker` · `cd web && pnpm dev`. See `Makefile`.

## Git

Commit per feature. Author `ebnsina <ebnsina.me@gmail.com>`. Never commit `docs/`, `data/`, or `.env`.
