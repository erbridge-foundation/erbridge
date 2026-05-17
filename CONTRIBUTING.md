# Contributing to erbridge

## Prerequisites

- **Rust** (stable toolchain via [rustup](https://rustup.rs/))
- **PostgreSQL 16** running locally (any install method: native package, Postgres.app, Homebrew, your own Docker container)
- **`sqlx-cli`** for migrations and the offline query cache:
  ```sh
  cargo install sqlx-cli --no-default-features --features postgres,rustls --locked
  ```

## Backend setup

### 1. Provision the database

The backend tests use [`#[sqlx::test]`](https://docs.rs/sqlx/latest/sqlx/attr.test.html), which creates a fresh database per test on top of a base database you provide. That base database must be **owned by** the role you connect with, and the role must have the `CREATEDB` privilege.

Connect to your local Postgres as a superuser (e.g. `psql -U postgres`) and run:

```sql
CREATE ROLE erbridge WITH LOGIN PASSWORD 'Passw0rd' CREATEDB;
CREATE DATABASE erbridge OWNER erbridge;
```

> If you prefer different credentials, you can override `DATABASE_URL` per-shell or in your own `.env` — see [Database configuration](#database-configuration) below.

### 2. Apply migrations

```sh
cd backend
sqlx migrate run
```

### 3. Configure app secrets

The application binary (`cargo run`) reads its config from `backend/.env`:

```sh
cp .env.example .env
# Edit .env: set ENCRYPTION_SECRET, ESI_CLIENT_ID, ESI_CLIENT_SECRET
```

Generate an encryption secret with `openssl rand -hex 32`. Register an ESI application at <https://developers.eveonline.com> to get the client credentials.

### 4. Run the tests

```sh
cargo test
```

That's it — no shell exports, no wrapper scripts.

## Database configuration

`cargo test` (and `cargo run`, `cargo check`, etc.) get `DATABASE_URL` from `backend/.cargo/config.toml`, which is committed to the repository. The default value points at the role and database created in step 1:

```
postgres://erbridge:Passw0rd@localhost/erbridge?sslmode=disable
```

A real shell environment variable always overrides the cargo default, so contributors using different credentials don't need to touch the committed file:

```sh
DATABASE_URL=postgres://me:secret@db.local/erbridge cargo test
```

The `.env` file is only read by the application binary at runtime (via `dotenvy` in `main.rs`); tests build their own in-memory config and only need `DATABASE_URL` to reach the database.

## The `.sqlx/` offline cache

`sqlx::query!` macros validate SQL against a live database at compile time. To let CI (and contributors without a running database) build the project, we commit a cache of those macro expansions to `backend/.sqlx/`.

**Whenever you add, remove, or change a `sqlx::query!` / `sqlx::query_as!` invocation, regenerate the cache:**

```sh
cd backend
cargo sqlx prepare
git add .sqlx
```

CI runs `cargo sqlx prepare --check` and fails the build if the cache is out of date.

## Continuous integration

`.github/workflows/backend.yml` runs `fmt`, `clippy`, the `.sqlx` drift check, and `cargo test` on every PR and push to `main` / `develop` that touches `backend/`. The workflow spins up a Postgres 16 service container and uses the built-in `postgres` superuser (which already has `CREATEDB` and owns the `postgres` database), so no extra DB bootstrapping is needed there.
