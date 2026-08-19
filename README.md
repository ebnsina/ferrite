# ferrite

Video transcoding: upload a file, get a link that plays anywhere, or get a
converted file back.

## Status

**Stage 0 — foundations**

| | |
|---|---|
| Rust workspace | 7 crates, dependency direction pinned in `Cargo.toml` |
| `ferrite-av` | probe + encoder backend seam, compiling against FFmpeg |
| Encoder backend interface | CPU (`x264`/`x265`) the only implementation |
| docker-compose | Postgres ×2, Temporal, MinIO, OTel, Prometheus, Grafana |
| OpenTelemetry | `ferrite-telemetry`, one init for every binary |
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
| Contact sheet | 60 frames, 10×6 grid, one JPEG — what a reviewer opens |
| Perceptual hash | 64-bit dHash per sampled frame, Hamming ≤ 10 holds |
| Quality | VMAF, PSNR, SSIM, MS-SSIM, CIEDE2000, CAMBI in one libvmaf pass |
| Audio | one AAC stereo track, encoded once, never chunked |
| Job mode | one file in, one file out, sharing steps 1–3 with asset mode |
| Corpus | eleven awkward files, a JSON report, and a diff that gates a merge |
| Not yet | thumbnails, `conform` against external validators |

## Get started

```sh
cp .env.example .env    # optional; only to move ports
make up                 # Postgres x2, Temporal, MinIO, OTel, Prometheus, Grafana
make test               # without FFmpeg, then with
make test-integration   # needs the stack up
make spike              # 1,000 steps through Temporal
cargo run -p ferrite-cli --features ffmpeg -- doctor
```

To watch the scheduler move real work, in two terminals:

```sh
make scheduler          # admission loop + internal API on :8081
make fake-worker        # runs ferrite.fake, reports completion back
```

`make ffmpeg` builds the pinned FFmpeg into `vendor/ffmpeg`; until then the
crate links against whatever `pkg-config` finds. `make packager` fetches the
pinned Shaka Packager into `vendor/packager`, which `ferrite package` needs.

Local pipeline, end to end:

```sh
ferrite probe   input.mp4          # codecs, duration, keyframes, problems
ferrite ladder  input.mp4          # which rungs, and why
ferrite split   input.mp4          # where every cut lands
ferrite encode  input.mp4 -o out/  # the ladder, decode-once
ferrite verify  out/               # frame counts, keyframe alignment, duration
ferrite package out/ -o cmaf/      # CMAF + HLS + DASH over one segment set
ferrite sheet   input.mp4          # contact sheet + a pHash per sampled frame
ferrite quality mezz.mp4 out/1080p.mp4 --min-vmaf 93
ferrite job    input.mp4 -o out.mp4 --height 720   # job mode: one output
ferrite bench  testdata/corpus -o bench.json       # the corpus report
ferrite compare before.json after.json             # the CI gate
```

`ferrite quality` needs an ffmpeg built `--enable-libvmaf`. Compare against the
mezzanine, never another encode: two encodes differing tells you nothing about
which is correct.

## Ports

Deliberately off the well-known ones, so this stack does not fight another on
the same machine. Copy `.env.example` to `.env` to change any of them.

| | | override |
|---|---|---|
| `assets_db` | 55432 | `FERRITE_ASSETS_DB_PORT` |
| `sched_db` | 55433 | `FERRITE_SCHED_DB_PORT` |
| Temporal | 7253 | `FERRITE_TEMPORAL_PORT` |
| Temporal UI | 8253 | `FERRITE_TEMPORAL_UI_PORT` |
| MinIO | 9020, console 9021 | `FERRITE_MINIO_PORT` |
| OTLP | 4327 gRPC, 4328 HTTP | `FERRITE_OTLP_GRPC_PORT` |
| Prometheus | 9092 | `FERRITE_PROMETHEUS_PORT` |
| Grafana | 3021 | `FERRITE_GRAFANA_PORT` |

## The unsafe rule

All `unsafe` lives in `ferrite-av`. Everywhere else the workspace lints set
`unsafe_code = "forbid"`.

## Layout

```
crates/ferrite-av          FFmpeg wrapper, encoder backend seam
crates/ferrite-telemetry   tracing + OpenTelemetry
crates/ferrite-scheduler   admission control, the internal API, the engine seam
crates/ferrite-worker      the machines converting video
crates/ferrite-cli         the `ferrite` binary
spike/temporal-spike     throwaway; proves the Temporal SDK

`ferrite-assets` (public API) and `ferrite-keys` (AES custody) arrive with the
stages that need them — Stage 2 and Stage 5.
```
