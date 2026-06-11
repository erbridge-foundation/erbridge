FROM rust:1.96.0-slim-bookworm AS builder

WORKDIR /app

# utoipa-swagger-ui's build.rs shells out to `curl` to fetch the Swagger UI
# bundle; the -slim base image does not ship it.
RUN apt-get update && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY migrations ./migrations
COPY .sqlx ./.sqlx

ENV SQLX_OFFLINE=true

# Git-tag-derived version + short commit, computed outside the image (the build
# context has no .git/) and passed in by CI / `just docker-build-backend`. build.rs
# reads these envs: APP_VERSION overrides CARGO_PKG_VERSION, GIT_COMMIT_SHA is baked
# in. With no build args they default empty → manifest version + "unknown" commit.
# See RELEASING.md.
ARG APP_VERSION=""
ARG GIT_COMMIT_SHA=""
ENV APP_VERSION=${APP_VERSION}
ENV GIT_COMMIT_SHA=${GIT_COMMIT_SHA}

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/backend ./backend

EXPOSE 3000

CMD ["./backend"]
