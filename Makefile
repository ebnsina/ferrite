DB_URL ?= postgres://ferrite:ferrite@localhost:5432/ferrite

.PHONY: help up down logs migrate api worker web build check fmt

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-10s\033[0m %s\n", $$1, $$2}'

up: ## Start Postgres, Redis, MinIO
	docker compose up -d

down: ## Stop infra
	docker compose down

logs: ## Tail infra logs
	docker compose logs -f

migrate: ## Apply database migrations
	sqlx migrate run --database-url "$(DB_URL)"

api: ## Run the API
	cargo run -p ferrite-api

worker: ## Run a transcode worker
	cargo run -p ferrite-worker

web: ## Run the dashboard dev server
	cd web && pnpm dev

build: ## Build everything
	cargo build && cd web && pnpm build

check: ## Type/lint check
	cargo clippy --all-targets && cd web && pnpm check

fmt: ## Format code
	cargo fmt && cd web && pnpm exec prettier --write .
