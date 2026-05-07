.PHONY: help dev down logs migrate cli test-oidc

help:
	@echo ""
	@echo "  make dev        — build and start postgres + ovlt-core (Docker)"
	@echo "  make down       — stop and remove containers"
	@echo "  make logs       — tail server logs"
	@echo "  make migrate    — run pending migrations (server must be stopped)"
	@echo "  make cli        — launch the Ratatui admin TUI"
	@echo "  make test-oidc  — run the full OIDC flow end-to-end (needs server up)"
	@echo ""

dev:
	docker compose up --build

down:
	docker compose down

logs:
	docker compose logs -f ovlt-core

migrate:
	docker compose run --rm ovlt-core ./ovlt-core

cli:
	@command -v cargo >/dev/null || { echo "cargo not found"; exit 1; }
	cargo run -p ovlt-cli connect $${OVLT_URL:-http://localhost:3000} && cargo run -p ovlt-cli serve

test-oidc:
	@bash scripts/test-oidc.sh
