.PHONY: help build run dev test fmt clippy check docker-build docker-up docker-down db-up db-down migrate create-admin clean

help:
	@echo "Available commands:"
	@echo "  make build         - Build release binary"
	@echo "  make dev           - Run locally with debug log"
	@echo "  make test          - Run cargo unit and integration tests"
	@echo "  make fmt           - Format codebase"
	@echo "  make clippy        - Run clippy linter"
	@echo "  make check         - Run complete CI local verification suite"
	@echo "  make docker-up     - Launch API & Postgres via Docker Compose"
	@echo "  make docker-down   - Stop Docker Compose services"
	@echo "  make create-admin  - Run create_admin binary"

build:
	cargo build --release

dev:
	cargo run

test:
	cargo test

fmt:
	cargo fmt

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

check: fmt clippy test build

docker-build:
	docker compose build

docker-up:
	docker compose up -d

seed-db:
	sqlx migrate run

docker-down:
	docker compose down

db-up:
	docker compose up -d postgres

db-down:
	docker compose stop postgres

create-admin:
	cargo run --bin create_admin

clean:
	cargo clean
