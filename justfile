set dotenv-load

image := "ghcr.io/erbridge-foundation/erbridge-api"

repo    := "erbridge-foundation/erbridge"

# Run all tests
test: test-backend # test-frontend test-e2e

# Run backend tests
test-backend:
    cd backend && cargo test --all-targets

# Run frontend tests
test-frontend:
    cd frontend && pnpm run check && pnpm run test

# Run end-to-end tests
test-e2e:
    cd frontend && pnpm run test:e2e

# Build both Docker images locally
docker-build: docker-build-backend docker-build-frontend

# Build the backend Docker image locally
docker-build-backend:
    docker build \
        --build-arg VERSION={{version}} \
        -t {{repo}}-backend:{{version}} \
        -t {{repo}}-backend:latest \
        ./backend

# Build the frontend Docker image locally
docker-build-frontend:
    docker build \
        --build-arg VERSION={{version}} \
        -t {{repo}}-frontend:{{version}} \
        -t {{repo}}-frontend:latest \
        ./frontend

# Regenerate the .sqlx offline query cache (run after changing any sqlx::query! macro)
prepare:
    cd backend && cargo sqlx prepare -- --tests
