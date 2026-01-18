.PHONY: help build run test clean docker-build docker-up docker-down migrate fmt lint

help:
	@echo "Hanzo Cloud Backend - Make Commands"
	@echo ""
	@echo "Development:"
	@echo "  make build        - Build the project"
	@echo "  make run          - Run the server locally"
	@echo "  make test         - Run tests"
	@echo "  make watch        - Run with auto-reload (requires cargo-watch)"
	@echo ""
	@echo "Code Quality:"
	@echo "  make fmt          - Format code"
	@echo "  make lint         - Run clippy"
	@echo "  make check        - Check without building"
	@echo ""
	@echo "Docker:"
	@echo "  make docker-build - Build Docker image"
	@echo "  make docker-up    - Start services with docker-compose"
	@echo "  make docker-down  - Stop services"
	@echo "  make docker-logs  - View service logs"
	@echo ""
	@echo "Database:"
	@echo "  make migrate      - Run database migrations"
	@echo "  make db-create    - Create database"
	@echo "  make db-drop      - Drop database"
	@echo ""
	@echo "Cleanup:"
	@echo "  make clean        - Clean build artifacts"

build:
	cargo build

run:
	cargo run

test:
	cargo test

watch:
	cargo watch -x run

fmt:
	cargo fmt

lint:
	cargo clippy -- -D warnings

check:
	cargo check

docker-build:
	docker build -t hanzo-cloud:latest .

docker-up:
	docker compose up -d

docker-down:
	docker compose down

docker-logs:
	docker compose logs -f cloud

migrate:
	sqlx migrate run

db-create:
	createdb hanzo_cloud

db-drop:
	dropdb hanzo_cloud

clean:
	cargo clean
	rm -rf target/
