# Ferrite's Docker Postgres is on 5455 (see FERRITE_PG_PORT in .env); 5432 is a
# separate native Postgres on this machine — don't point migrations at it.
DB_URL ?= postgres://ferrite:ferrite@localhost:5455/ferrite

.PHONY: help up down logs migrate api worker web build check fmt fresh kill

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

fresh: ## Kill Ferrite's own procs/ports and restart API+worker+web cleanly
	./scripts/fresh.sh

kill: ## Stop only Ferrite's processes (API 8787, web 5173) — leaves other projects alone
	-pkill -f 'target/debug/ferrite-api' 2>/dev/null
	-pkill -f 'target/debug/ferrite-worker' 2>/dev/null
	-pkill -f 'Sites/ferrite/web/node_modules' 2>/dev/null
	-lsof -tiTCP:8787 -sTCP:LISTEN 2>/dev/null | xargs -r kill 2>/dev/null
	-lsof -tiTCP:5173 -sTCP:LISTEN 2>/dev/null | xargs -r kill 2>/dev/null
	@echo "Ferrite stopped."

build: ## Build everything
	cargo build && cd web && pnpm build

check: ## Type/lint check
	cargo clippy --all-targets && cd web && pnpm check

fmt: ## Format code
	cargo fmt && cd web && pnpm exec prettier --write .
