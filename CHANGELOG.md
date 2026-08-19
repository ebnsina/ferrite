# Changelog

Notable changes to Ferrite. Newest first.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [semantic versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Renamed the project from Verve to Ferrite: crates, binaries, environment
  variables, container names and buckets.
- Every environment variable is now required. `ferrite-worker` storage and
  `ferrite-telemetry` config fail at startup rather than defaulting a region,
  an endpoint or a log level. The CLI still runs unconfigured, having nowhere
  to send telemetry and nobody reading it.

- Row-level security on every tenant-scoped table in `sched_db`, forced so the
  owner is subject to it too. Each query declares a scope: one tenant, or the
  cross-tenant service scope that admission needs. A query with no scope returns
  nothing.
- The application connects as `ferrite_app`, a non-superuser role. Superusers
  bypass row-level security outright, which made the policies decorative.

### Fixed

- `probe` refused any file without a video stream, so it could not read the
  audio renditions we produce ourselves. Audio-only is now legitimate; only a
  file with neither pictures nor sound is refused.
- A clean silent video reported that it needed a mezzanine. Only problems a
  normalising pass actually fixes count now; silence, an unknown duration and a
  missing keyframe index do not.

- Anamorphic sources were laddered from their coded size, so a 720×576 frame
  carrying a 16:9 picture came out 600×480. Display size now applies the sample
  aspect ratio before rotation.
- Every HDR rendition was flagged for backwards timestamps. The probe checked
  PTS monotonicity in decode order, which every B-frame violates by design; it
  now checks decode timestamps.

## Stage 2 — one machine, end to end

### Added

- Audio normalisation: one AAC stereo track, encoded once and shared by every
  video rung, never chunked. Source audio was passed straight through before,
  which the packager refuses outright for an MP3 soundtrack in an MP4.
- Job mode: one file in, one file out, on the same `Source` path as asset mode.
  Never upscales past the source, and still samples frames for the blocklist —
  there is no mezzanine to hang that off, and the alternative is publishing
  files nobody has looked at.

- Ladder planning: never upscales, drops rungs above the source bitrate, keeps
  the source aspect ratio.
- Split planning: cuts only at keyframes, leaves sources under two minutes
  whole, and records why.
- Decode-once transcode: one decode feeds every rung, with rotation, frame rate,
  scale and pixel format through libavfilter.
- Check step and `ferrite verify`: catches the dropped chunk that still plays.
- Packaging to CMAF with HLS and DASH over one segment set, via Shaka Packager.
- Contact sheet and a 64-bit perceptual hash per sampled frame.
- `ferrite quality`: VMAF, PSNR, SSIM, MS-SSIM, CIEDE2000 and CAMBI in one pass.
- `ferrite bench` and `ferrite compare`: the corpus report and the CI gate.

## Stage 1 — scheduling

### Added

- `work` table with per-tenant dedupe and `FOR UPDATE SKIP LOCKED` claims.
- Admission loop with lane guarantees and proportional fairness across tenants.
- Recovery for a scheduler killed between claim and start.
- Internal API: work submission, cancellation, completion, budgets, capacity.
- Temporal behind a workflow-engine trait, started untyped by name.

## Stage 0 — foundations

### Added

- Rust workspace, with all `unsafe` confined to `ferrite-av`.
- Encoder backend seam with a CPU implementation.
- docker-compose: Postgres ×2, Temporal, MinIO, OpenTelemetry, Prometheus,
  Grafana.
- Ansible worker role with CPU pinning and FFmpeg sandboxing.
