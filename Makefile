# Stage 0 targets. `make check` is what CI runs.
SHELL := /usr/bin/env bash
FEATURES ?= ffmpeg

.PHONY: help
help:
	@grep -E '^[a-z-]+:.*##' $(MAKEFILE_LIST) | sed 's/:.*##/\t/' | column -t -s "$$(printf '\t')"

.PHONY: up
up: ## Start Postgres x2, Temporal, MinIO, OTel, Prometheus, Grafana
	docker compose up -d

.PHONY: down
down: ## Stop everything, keep volumes
	docker compose down

.PHONY: nuke
nuke: ## Stop everything and delete volumes
	docker compose down -v

.PHONY: ffmpeg
ffmpeg: ## Build the pinned FFmpeg into vendor/ffmpeg
	./scripts/build-ffmpeg.sh

.PHONY: build
build: ## Build the workspace with FFmpeg
	cargo build --workspace --features verve-av/$(FEATURES)

.PHONY: test
test: ## Test without FFmpeg, then with
	cargo test --workspace
	cargo test -p verve-av --features $(FEATURES)

.PHONY: test-integration
test-integration: ## Tests needing the compose stack up
	VERVE_S3_BUCKET=verve-assets \
	VERVE_S3_ENDPOINT=http://localhost:9000 \
	VERVE_S3_REGION=us-east-1 \
	VERVE_S3_ACCESS_KEY=verve \
	VERVE_S3_SECRET_KEY=verve-dev-secret \
	cargo test -p verve-worker --test minio_roundtrip

.PHONY: spike
spike: ## 1,000 steps across child workflows. Needs `make up`.
	cargo run -p temporal-spike -- --steps 1000 --chunk 50 --run-id $$(date +%s)

.PHONY: fmt
fmt: ## Format
	cargo fmt --all

.PHONY: check
check: ## What CI runs
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace
