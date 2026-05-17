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

# Apply migrations once so `sqlx::query!` macros can validate against a live
# schema during test compilation. `#[sqlx::test]` still creates and migrates a
# fresh per-test database on top of this baseline.
for migration in migrations/*.sql; do
  docker compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" exec -T \
    -e PGPASSWORD=erbridge_test postgres-test \
    psql -U erbridge_test -d erbridge_test -v ON_ERROR_STOP=1 \
    <"$migration" >/dev/null
done

cargo test "$@"
