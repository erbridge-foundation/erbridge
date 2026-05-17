#!/usr/bin/env bash
# Run the backend test suite against an ephemeral Postgres container.
#
# Spins up backend/docker-compose.test.yml, waits for the database to become
# healthy, exports DATABASE_URL pointing at it, runs `cargo test`, and tears
# the container down on exit. Any arguments are forwarded to `cargo test`.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKEND_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
COMPOSE_FILE="$BACKEND_DIR/docker-compose.test.yml"
PROJECT_NAME="erbridge-backend-test"

cleanup() {
  docker compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" down --volumes --remove-orphans >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" up -d --wait

export DATABASE_URL="postgres://erbridge_test:erbridge_test@localhost:55432/erbridge_test"

cd "$BACKEND_DIR"
cargo test "$@"
