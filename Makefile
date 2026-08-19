# Stage 0 targets. `make check` is what CI runs.
SHELL := /usr/bin/env bash
FEATURES ?= ffmpeg
# Match .env.example. Override per-invocation or in your own .env.
FERRITE_SCHED_DB_PORT ?= 55433
FERRITE_MINIO_PORT ?= 9020
FERRITE_TEMPORAL_PORT ?= 7253
SCHED_DATABASE_URL ?= postgres://ferrite_app:ferrite-app-dev@localhost:$(FERRITE_SCHED_DB_PORT)/sched_db
export SCHED_DATABASE_URL

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

.PHONY: packager
packager: ## Fetch the pinned Shaka Packager into vendor/
	./scripts/fetch-packager.sh

.PHONY: build
build: ## Build the workspace with FFmpeg
	cargo build --workspace --features ferrite-av/$(FEATURES)

.PHONY: test
test: ## Test without FFmpeg, then with
	cargo test --workspace
	cargo test -p ferrite-av --features $(FEATURES)

.PHONY: test-integration
test-integration: ## Tests needing the compose stack up
	FERRITE_S3_BUCKET=ferrite-assets \
	FERRITE_S3_ENDPOINT=http://localhost:$(FERRITE_MINIO_PORT) \
	FERRITE_S3_REGION=us-east-1 \
	FERRITE_S3_ACCESS_KEY=ferrite \
	FERRITE_S3_SECRET_KEY=ferrite-dev-secret \
	cargo test -p ferrite-worker --test minio_roundtrip
	cargo test -p ferrite-scheduler --tests

.PHONY: scheduler
scheduler: ## Run the scheduler against Temporal. Needs `make up`.
	cargo run -p ferrite-scheduler --features temporal -- --total-slots 8 \
		--temporal-address http://localhost:$(FERRITE_TEMPORAL_PORT)

.PHONY: fake-worker
fake-worker: ## Run a worker for ferrite.fake. Needs `make scheduler`.
	cargo run -p ferrite-scheduler --features fake-worker --bin ferrite-fake-worker -- \
		--address http://localhost:$(FERRITE_TEMPORAL_PORT)

.PHONY: spike
spike: ## 1,000 steps across child workflows. Needs `make up`.
	cargo run -p temporal-spike -- --address http://localhost:$(FERRITE_TEMPORAL_PORT) \
		--steps 1000 --chunk 50 --run-id $$(date +%s)

.PHONY: fmt
fmt: ## Format
	cargo fmt --all

.PHONY: check
check: ## What CI runs
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace
