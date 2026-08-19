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

- Segments no longer carry a `sidx` index. It is only required for the
  on-demand profile, which addresses by byte range; we address by template, so
  it was bytes in every segment for nothing and made each one claim indexing it
  did not have.
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

## Stage 3 — chunking

### Added

- Chunked encoding: the source is split at keyframes and each piece encoded
  separately, with one decode per chunk still feeding every rung. Time then
  depends on how many machines are free rather than on how long the video is.
- `join`: concatenates compressed data and fixes timestamps. Nothing is
  re-encoded — decoding a joined chunk would throw away the quality chunking
  exists to preserve.
- `ferrite run --whole` encodes in one pass, for comparing against.

### Measured

Against a 150s source, chunked versus whole: VMAF 97.579 against 97.429, worst
frame identical, CAMBI 0.4008 against 0.4015, CIEDE2000 57.95 against 57.59.
Frame counts and durations match exactly, and keyframes stay identical across
every rung after the join. The gate is a drop of no more than 0.5 VMAF; chunking
came out marginally ahead.

## Stage 2 — one machine, end to end

### Added

- `scripts/play.sh`: serves a published asset with hls.js and dash.js side by
  side. Conformance tools prove the files are correct; this proves they play.
- Audio renditions carry the source's language when it states one. The packager
  rejects `und`, so an unknown language is left unclaimed rather than guessed at.
- `ferrite conform`: MPEG-DASH and CMAF standards checks against DASH-IF,
  pinned to 2.4.1 and run as a container. Manifest-level checks pass. Segment
  checks report seven findings from the validator's ISO/IEC 23009-1:2012 rules,
  which CMAF supersedes — recorded rather than silenced.
- `ferrite run`: the whole pipeline end to end on one machine — probe, ladder,
  decode-once encode, audio, checks, contact sheet, thumbnails and packaging.
  Publishing is gated on the checks passing.
- Thumbnails, written from the same decode as the contact sheet and named by
  the source time they were taken from.
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
