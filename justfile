set dotenv-load

# Image names match the GHCR registry layout: ghcr.io/erbridge-foundation/{api,ui}
registry := "ghcr.io/erbridge-foundation"

# ─── default: list available recipes ─────────────────────────────────────────
default:
    @just --list

# ─── full gate (CI parity) ───────────────────────────────────────────────────
# Runs every gate both CIs run (lint + test, backend + frontend). Use this
# before pushing.
check: check-backend check-frontend

# Backend lint + sqlx cache freshness + all tests
check-backend: backend-fmt backend-clippy backend-sqlx-check test-backend

# Frontend type check + all tests
check-frontend: frontend-check test-frontend

# ─── test recipes ────────────────────────────────────────────────────────────

# Run all tests (backend unit + integration, frontend unit + e2e)
test: test-backend test-frontend

# Backend unit + integration tests
test-backend:
    cd backend && cargo test --all-targets

# Frontend unit tests (vitest) + e2e tests (Playwright)
test-frontend: test-frontend-unit test-frontend-e2e

# Frontend unit tests only (vitest)
test-frontend-unit:
    cd frontend && pnpm run test

# Frontend e2e tests only (Playwright)
test-frontend-e2e:
    cd frontend && pnpm run test:e2e

# ─── individual lint / type-check recipes ────────────────────────────────────

# Backend formatting check
backend-fmt:
    cd backend && cargo fmt --all -- --check

# Backend lints (deny-warnings)
backend-clippy:
    cd backend && cargo clippy --all-targets -- -D warnings

# Backend sqlx offline-query-cache freshness check
backend-sqlx-check:
    cd backend && cargo sqlx prepare --check -- --all-targets

# Frontend type check (svelte-check)
frontend-check:
    cd frontend && pnpm run check

# ─── docker ──────────────────────────────────────────────────────────────────

# Build both Docker images locally
docker-build: docker-build-backend docker-build-frontend

# Git-tag-derived version (leading "v" stripped; 0.0.0-dev.<sha> when no tag yet)
# and short commit, computed from the local checkout. The Docker build context has
# no .git/, so these are passed in as --build-arg. See RELEASING.md.
_app-version:
    #!/usr/bin/env sh
    sha="$(git rev-parse --short HEAD)"
    if git describe --tags --abbrev=0 >/dev/null 2>&1; then
        described="$(git describe --tags --always --dirty)"
        echo "${described#v}"
    else
        echo "0.0.0-dev.${sha}"
    fi

# Build the backend Docker image locally
docker-build-backend:
    docker build \
        --build-arg APP_VERSION="$(just _app-version)" \
        --build-arg GIT_COMMIT_SHA="$(git rev-parse --short HEAD)" \
        -t {{registry}}/erbridge-api:latest ./backend

# Build the frontend Docker image locally
docker-build-frontend:
    docker build \
        --build-arg APP_VERSION="$(just _app-version)" \
        --build-arg GIT_COMMIT_SHA="$(git rev-parse --short HEAD)" \
        -t {{registry}}/erbridge-web:latest ./frontend

# ─── maintenance ─────────────────────────────────────────────────────────────

# Regenerate the .sqlx offline query cache (after changing any sqlx::query! macro)
sqlx-prepare:
    cd backend && cargo sqlx prepare -- --tests
