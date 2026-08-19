# verve

Video transcoding: upload a file, get a link that plays anywhere, or get a
converted file back. See [docs/](docs/) — [overview](docs/01-overview.md),
[architecture](docs/02-architecture.md), [stages](docs/07-implementation.md).

## Status

**Stage 0 — foundations**

| | |
|---|---|
| Rust workspace | 7 crates, dependency direction pinned in `Cargo.toml` |
| `verve-av` | probe + encoder backend seam, compiling against FFmpeg |
| Encoder backend interface | CPU (`x264`/`x265`) the only implementation |
| docker-compose | Postgres ×2, Temporal, MinIO, OTel, Prometheus, Grafana |
| OpenTelemetry | `verve-telemetry`, one init for every binary |
| Ansible | `deploy/ansible` — worker role, systemd, CPU pinning, sandboxing |
| Temporal spike | 1,000 steps across 20 child workflows, exactly-once |

**Stage 1 — scheduling**, proven on fake work before any video code exists

| | |
|---|---|
| `work` table | dedupe per tenant, `FOR UPDATE SKIP LOCKED`, weight copied at submit |
| Admission loop | lane guarantees, then proportional share across tenants |
| Fairness | weight sets how *fast*, never *whether* — 10,000 items cannot starve one |
| Recovery | a scheduler killed between claim and start loses and duplicates nothing |
| Cancellation | reaches the running workflow, then releases the slot |
| Cost | CPU seconds and bytes per task, ready for billing to roll up |
| Internal API | `/internal/work`, `/cancel`, `/finish`, `/budgets`, `/capacity` |
| Temporal | behind the engine trait; workflows started untyped, by name |

**Stage 2 — one machine, end to end** (in progress)

| | |
|---|---|
| Ladder | never upscales, drops rungs above the source bitrate, keeps the aspect |
| Split plan | cuts only at keyframes, leaves short sources whole, reproducible |
| Transcode | one decode feeds every rung; rotation, fps, scale via libavfilter |
| Check | catches a dropped chunk — the file that still plays but is wrong |
| Package | CMAF + HLS + DASH over one segment set, via Shaka Packager |
| Not yet | audio encode, thumbnails, contact sheet, pHash, job mode, `quality`/`bench` |

## Get started

```sh
cp .env.example .env    # optional; only to move ports
make up                 # Postgres x2, Temporal, MinIO, OTel, Prometheus, Grafana
make test               # without FFmpeg, then with
make test-integration   # needs the stack up
make spike              # 1,000 steps through Temporal
cargo run -p verve-cli --features ffmpeg -- doctor
```

To watch the scheduler move real work, in two terminals:

```sh
make scheduler          # admission loop + internal API on :8081
make fake-worker        # runs verve.fake, reports completion back
```

`make ffmpeg` builds the pinned FFmpeg into `vendor/ffmpeg`; until then the
crate links against whatever `pkg-config` finds. `make packager` fetches the
pinned Shaka Packager into `vendor/packager`, which `verve package` needs.

Local pipeline, end to end:

```sh
verve probe   input.mp4          # codecs, duration, keyframes, problems
verve ladder  input.mp4          # which rungs, and why
verve split   input.mp4          # where every cut lands
verve encode  input.mp4 -o out/  # the ladder, decode-once
verve verify  out/               # frame counts, keyframe alignment, duration
verve package out/ -o cmaf/      # CMAF + HLS + DASH over one segment set
```

## Ports

Deliberately off the well-known ones, so this stack does not fight another on
the same machine. Copy `.env.example` to `.env` to change any of them.

| | | override |
|---|---|---|
| `assets_db` | 55432 | `VERVE_ASSETS_DB_PORT` |
| `sched_db` | 55433 | `VERVE_SCHED_DB_PORT` |
| Temporal | 7253 | `VERVE_TEMPORAL_PORT` |
| Temporal UI | 8253 | `VERVE_TEMPORAL_UI_PORT` |
| MinIO | 9020, console 9021 | `VERVE_MINIO_PORT` |
| OTLP | 4327 gRPC, 4328 HTTP | `VERVE_OTLP_GRPC_PORT` |
| Prometheus | 9092 | `VERVE_PROMETHEUS_PORT` |
| Grafana | 3021 | `VERVE_GRAFANA_PORT` |

## The unsafe rule

All `unsafe` lives in `verve-av`. Everywhere else the workspace lints set
`unsafe_code = "forbid"`.

## Layout

```
crates/verve-av          FFmpeg wrapper, encoder backend seam
crates/verve-telemetry   tracing + OpenTelemetry
crates/verve-scheduler   admission control, the internal API, the engine seam
crates/verve-worker      the machines converting video
crates/verve-cli         the `verve` binary
spike/temporal-spike     throwaway; proves the Temporal SDK

`verve-assets` (public API) and `verve-keys` (AES custody) arrive with the
stages that need them — Stage 2 and Stage 5.
```
