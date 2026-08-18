# verve

Video transcoding: upload a file, get a link that plays anywhere, or get a
converted file back. See [docs/](docs/) — [overview](docs/01-overview.md),
[architecture](docs/02-architecture.md), [stages](docs/07-implementation.md).

## Status — Stage 0, foundations

| | |
|---|---|
| Rust workspace | 7 crates, dependency direction pinned in `Cargo.toml` |
| `verve-av` | probe + encoder backend seam, compiling against FFmpeg |
| Encoder backend interface | CPU (`x264`/`x265`) the only implementation |
| docker-compose | Postgres ×2, Temporal, MinIO, OTel, Prometheus, Grafana |
| OpenTelemetry | `verve-telemetry`, one init for every binary |
| Ansible | `deploy/ansible` — worker role, systemd, CPU pinning, sandboxing |
| Temporal spike | 1,000 steps across 20 child workflows, exactly-once |

## Get started

```sh
make up                 # Postgres x2, Temporal, MinIO, OTel, Prometheus, Grafana
make test               # without FFmpeg, then with
make spike              # 1,000 steps through Temporal
cargo run -p verve-cli --features ffmpeg -- doctor
```

`make ffmpeg` builds the pinned FFmpeg into `vendor/ffmpeg`; until then the
crate links against whatever `pkg-config` finds.

## Ports

| | |
|---|---|
| `assets_db` | 55432 |
| `sched_db` | 55433 |
| Temporal | 7233, UI on 8233 |
| MinIO | 9000, console on 9001 |
| OTLP | 4317 gRPC, 4318 HTTP |
| Prometheus | 9090 |
| Grafana | 3001 |

## The unsafe rule

All `unsafe` lives in `verve-av`. Everywhere else the workspace lints set
`unsafe_code = "forbid"`.

## Layout

```
crates/verve-av          FFmpeg wrapper, encoder backend seam
crates/verve-keys        AES key generation and custody
crates/verve-telemetry   tracing + OpenTelemetry
crates/verve-assets      public API
crates/verve-scheduler   admission control
crates/verve-worker      the machines converting video
crates/verve-cli         the `verve` binary
spike/temporal-spike     throwaway; proves the Temporal SDK
```
