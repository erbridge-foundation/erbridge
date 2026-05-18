---
name: rust-rest-api
description: |
  Rules for the Rust REST API backend: layered architecture (handler → service → db),
  DTOs, response envelope, error handling, and full test coverage (unit + integration + HURL).
  TRIGGER when: starting work on any task in the `backend/` directory of this repo,
  including the first scaffolding tasks before files exist; editing files under
  backend/src/{handlers,services,db,dto}/ or backend/tests/; writing or modifying
  axum/sqlx code; adding HURL tests under tests/hurl/; reviewing a backend PR;
  designing a new endpoint or repository function; applying tasks from an OpenSpec
  change whose tasks.md mentions backend Rust files. Invoke before writing the
  first line of Rust in a session.
  SKIP: frontend code, infrastructure-only changes (Dockerfiles, Compose, Traefik
  config), migrations-only edits, or documentation changes that don't touch
  handler/service/db code.
---

# Rust REST API — Rules & Guidance

## Module Layout

Code is organised by **layer**, not by domain. Every handler lives in `src/handlers/`, every service in `src/services/`, every DB function in `src/db/`. Files within each layer are named by domain/resource.

```
backend/src/
├── main.rs              # server entry point, router wiring
├── app_state.rs         # AppState struct
├── config.rs            # env-var loading, fail-fast
├── error.rs             # AppError enum + IntoResponse
├── response.rs          # ApiResponse<T> envelope type
├── session.rs           # Session + SessionStore
│
├── handlers/            # HANDLER LAYER — one file per route group
│   ├── mod.rs
│   ├── auth.rs          # /auth/* routes: login, callback, logout, add_character
│   └── api/
│       └── v1/
│           ├── mod.rs
│           ├── keys.rs
│           ├── me.rs
│           ├── characters.rs
│           └── account.rs
│
├── services/            # SERVICE LAYER — one file per domain
│   ├── mod.rs
│   ├── auth.rs          # SSO flow, session management
│   └── account.rs       # account + character business logic
│
├── db/                  # DATABASE LAYER — one file per resource
│   ├── mod.rs           # connect() + migration runner
│   ├── accounts.rs
│   ├── characters.rs
│   └── api_keys.rs
│
├── dto/                 # DTOs — one file per resource or handler group
│   └── *.rs
│
└── esi/                 # ESI client helpers (not a layer — external API client)
    ├── mod.rs           # EsiMetadata + discover()
    └── public_info.rs
```

**Rules enforced by this layout:**
- Never add a handler function outside `src/handlers/`.
- Never add a service function outside `src/services/`.
- Never add a DB function outside `src/db/`.
- Auth-related crypto/cookie helpers (`crypto.rs`, `cookie.rs`, `middleware.rs`) live in `src/handlers/auth_support/` or alongside `src/handlers/auth.rs` as sibling files in `src/handlers/` — they are handler-layer concerns, not services.

---

## Architecture: The Only Permitted Flow

```
HTTP Request
    │
    ▼
Handler  (src/handlers/)
    │  validates input, calls service, returns DTO wrapped in envelope
    ▼
Service  (src/services/)
    │  owns business logic, calls db layer
    ▼
DB / Repository  (src/db/)
    │  raw SQL or ORM queries, returns domain types
    ▼
Database
```

**This flow is strictly one-directional and may not be broken:**

- Handlers **must not** call `db` functions directly.
- Services **must not** return HTTP types (`StatusCode`, `Response`, etc.).
- DB functions **must not** contain business logic.
- No layer may import from a layer above it.

**Enforce this at the module boundary, not by review discipline.** Pick one of these mechanisms and stick to it:

1. **Visibility-driven** (preferred). The `db` module's items are `pub(super)` (visible to `services` only), and `services` items are `pub(super)` (visible to `handlers` only). Anything more permissive on a `db::*` symbol is a review-blocker.
2. **Workspace crates**. Split into `backend-db`, `backend-services`, `backend-handlers` crates with explicit `Cargo.toml` dependencies. The compiler refuses upward calls. Heaviest but bulletproof.
3. **Clippy / `cargo-deny` lint config**. Add a `disallowed-methods` / `disallowed-types` entry that rejects `crate::db::*` from `handlers::*`. CI fails on violation.

If none of these are in place, the architecture rule is aspirational only — call that out explicitly in the PR.

---

## Handler Rules

- Handlers live in `src/handlers/`.
- Each handler **must** accept injected state (e.g., `State<AppState>`) — no globals.
- Handlers **must** call exactly one service function per logical operation.
- Handlers **must** return a DTO (not a DB model) wrapped in the standard envelope.
- Validation of incoming request bodies happens in the handler before calling the service.
- Error mapping from service errors to HTTP responses happens in the handler.

```rust
// CORRECT
async fn create_user(
    State(state): State<AppState>,
    Json(body): Json<CreateUserRequest>,
) -> Result<Json<ApiResponse<UserDto>>, AppError> {
    body.validate()?;
    let user = state.user_service.create_user(body).await?;
    Ok(Json(ApiResponse::data(user)))
}

// WRONG — handler calling db directly
async fn create_user(State(state): State<AppState>, ...) {
    let user = db::users::insert(&state.db, ...).await?; // ❌
}
```

---

## Service Rules

- Services live in `src/services/`.
- Services **must not** import `axum`, or any HTTP framework types.
- Services own all business logic: validation that depends on persisted state, orchestration of multiple DB calls, etc.
- If a DB method can be slightly extended (e.g., return one extra column, add a `RETURNING` clause) to avoid a second round-trip, **extend the DB method** — do not write a second DB function. **Exception:** if extending would force unrelated callers to fetch significantly more data (large `TEXT` / `BYTEA` columns, a wide join), add a dedicated function instead. The rule is "one round-trip per operation", not "every caller pays for every reader's needs".
- Services return domain types or DTOs; never raw DB row types.

```rust
// CORRECT — extend the query, don't add a second db call
impl UserService {
    pub async fn activate_user(&self, id: Uuid) -> Result<UserDto, ServiceError> {
        // The db fn returns the updated row — no second fetch needed
        let user = self.db.users.activate(id).await?;
        Ok(UserDto::from(user))
    }
}

// WRONG — two db calls when one would do
let _ = self.db.users.activate(id).await?;      // ❌
let user = self.db.users.find_by_id(id).await?; // ❌ unnecessary second call
```

---

## DB / Repository Rules

- DB functions live in `src/db/`. One file per resource (`accounts.rs`, `characters.rs`, `api_keys.rs`). Never nest further.
- Each function maps to a query or a small, cohesive set of queries.
- **Before adding a new function**, check whether an existing one can be extended (e.g., add a `RETURNING` clause, join an extra table) to satisfy the new requirement.
- DB functions return domain model structs (`User`, `Order`, …) — never raw query row types exposed outside `src/db/`.
- No business logic inside DB functions. A DB function that takes a `status: &str` and validates it is wrong — validation belongs in the service.

---

## DTOs

- DTOs live in `src/dto/` (or co-located per feature — be consistent).
- Every handler response **must** use a DTO, never a DB model.
- Implement `From<DbModel> for Dto` — do not map fields inline in handlers or services.
- A DTO is an **explicit allowlist** of safe-to-serialise fields. Treat every field as an intentional decision to expose it.
- Never use `#[serde(flatten)]` to fold a DB model into a DTO — that smuggles every field of the DB type into the wire format, including any added later.
- Never `#[derive(Serialize)]` directly on a DB model (i.e. on the struct returned from `src/db/`). Serialisation is a DTO responsibility; DB models stay internal.
- Do not use `#[serde(skip_serializing_if = ...)]` on a sensitive field as a guard — conditional skip is not allowlisting; just don't include the field in the DTO.

```rust
#[derive(Serialize)]
pub struct UserDto {
    pub id: Uuid,
    pub email: String,
    pub created_at: DateTime<Utc>,
    // ← password_hash is NOT here
}

impl From<User> for UserDto {
    fn from(u: User) -> Self {
        Self { id: u.id, email: u.email, created_at: u.created_at }
    }
}
```

---

## API Response Envelope

All endpoints **except** `/api/healthz` must return:

```json
{ "data": <payload> }
```

For lists:
```json
{ "data": [ … ] }
```

For single items:
```json
{ "data": { … } }
```

### Envelope type

```rust
#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn data(payload: T) -> Self {
        Self { data: payload }
    }
}
```

### `/api/healthz` exception

The healthz endpoint returns its own structure — **no envelope**:

```json
{ "status": "ok", "version": "1.2.3" }
```

Do not wrap it. Do not apply `ApiResponse` to it.

---

## Error Handling

- Define a single `AppError` enum in `src/error.rs`.
- Implement `IntoResponse` for `AppError` so errors are converted at the handler boundary.
- Services return `ServiceError`; handlers map it to `AppError`.
- Never use `.unwrap()` or `.expect()` in handler, service, or DB code. In tests, `.unwrap()` is acceptable where the intent is to panic on unexpected failure.

---

## Testing Requirements

### Test environment

Backend tests run against the contributor's **local Postgres** (no Docker harness). `cargo test` picks up `DATABASE_URL` from `backend/.cargo/config.toml`; the role needs `CREATEDB` and must own the base database so `#[sqlx::test]` can spawn its per-test DBs and bookkeeping schema. Setup steps live in `CONTRIBUTING.md`.

`sqlx::query!` macros validate against the live DB at compile time. To keep `cargo build` working without a database (and for CI), the repo ships a committed `backend/.sqlx/` offline cache. **After adding, removing, or changing any `sqlx::query!` invocation, regenerate the cache with `cargo sqlx prepare -- --all-targets` from `backend/` and commit the diff.** The `--all-targets` flag is required so test-only `sqlx::query!` invocations (under `#[cfg(test)]` and in integration tests) are also cached — without it the cache drifts and CI fails on the next run. CI runs `cargo sqlx prepare --check -- --all-targets` and fails on drift.

### Unit Tests — 100% coverage of every non-trivial function

Unit-test coverage is **not** limited to the service layer. Every function with meaningful behaviour gets a unit test — handlers, services, DB functions, and helpers alike. The only exclusions are trivial glue (one-line `From` impls, a constructor that just assigns fields, a handler that does literally nothing but `service.call().await`). If a function has a branch, a transformation, a validation, or an error path, it needs a test.

Tests live in `#[cfg(test)]` modules within the file they cover (preferred — keeps the test next to the code), or in a sibling `tests.rs` for that module.

**Coverage targets per layer:**

| Layer | What to test | How to isolate |
|---|---|---|
| Handler | request parsing, validation dispatch, envelope shape, error → status mapping | mock the service (trait + test double) |
| Service | business logic, orchestration, error translation from DB to service errors | mock the DB layer (trait + `mockall` or hand-rolled) |
| DB | SQL correctness, row-to-domain mapping, constraint behaviour, transaction semantics | real test DB via `#[sqlx::test]` — unit-level scope, one function per test |
| Helper / pure functions | every branch and edge case | none needed — pure functions test directly |
| DTO `From` impls | only if the mapping is non-trivial (computed fields, conditional inclusion, redaction) | none needed |
| Error → response mapping | every `AppError` variant maps to the documented status & body | none needed; construct the variant and call `into_response` |

**Service example — mock the DB:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::MockUserDb;

    #[tokio::test]
    async fn create_user_hashes_password() {
        let mut mock_db = MockUserDb::new();
        mock_db.expect_insert().returning(|u| Ok(fake_user(u)));
        let svc = UserService::new(Arc::new(mock_db));
        let result = svc.create_user(valid_request()).await.unwrap();
        assert_ne!(result.password_hash, "plaintext");
    }
}
```

**Handler example — mock the service:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_user_returns_201_with_envelope() {
        let mut mock_svc = MockUserService::new();
        mock_svc.expect_create_user().returning(|_| Ok(fake_user_dto()));
        let state = AppState::with_user_service(Arc::new(mock_svc));
        let resp = create_user(State(state), Json(valid_request())).await.unwrap();
        assert_eq!(resp.0.data.email, "a@b.com");
    }

    #[tokio::test]
    async fn create_user_maps_conflict_to_409() {
        let mut mock_svc = MockUserService::new();
        mock_svc.expect_create_user().returning(|_| Err(ServiceError::Conflict));
        let state = AppState::with_user_service(Arc::new(mock_svc));
        let err = create_user(State(state), Json(valid_request())).await.unwrap_err();
        assert_eq!(err.into_response().status(), StatusCode::CONFLICT);
    }
}
```

**DB example — real test DB, one function under test:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn insert_then_find_returns_same_row(pool: PgPool) {
        let inserted = insert(&pool, NewUser { email: "a@b.com".into() }).await.unwrap();
        let found = find_by_id(&pool, inserted.id).await.unwrap();
        assert_eq!(found.email, "a@b.com");
    }

    #[sqlx::test]
    async fn insert_duplicate_email_returns_unique_violation(pool: PgPool) {
        insert(&pool, NewUser { email: "a@b.com".into() }).await.unwrap();
        let err = insert(&pool, NewUser { email: "a@b.com".into() }).await.unwrap_err();
        assert!(matches!(err, DbError::UniqueViolation { .. }));
    }
}
```

**Helper example — pure function, no mocks:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_email_lowercases_and_trims() {
        assert_eq!(normalize_email("  A@B.COM  "), "a@b.com");
    }

    #[test]
    fn normalize_email_rejects_missing_at() {
        assert!(normalize_email_checked("bogus").is_err());
    }
}
```

Note the boundary with integration tests: a *unit* DB test exercises one function in isolation; an *integration* test exercises a full handler→service→db request. Both use `#[sqlx::test]`; the distinguishing factor is scope, not tooling.

### Integration Tests — 100% coverage of handler→service→db paths

- Live in `tests/` at the project root.
- Use a real (test) database — spin up via `sqlx::test` or a Docker fixture.
- Every handler must be exercised end-to-end at least once.
- Test both happy paths and key error paths (not found, validation failure, conflict).

```rust
use tower::ServiceExt; // brings `.oneshot()` into scope on `axum::Router`

#[sqlx::test]
async fn test_create_user_returns_dto_envelope(pool: PgPool) {
    let app = build_test_app(pool);
    let resp = app.oneshot(post_json("/api/users", json!({"email": "a@b.com"}))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let body: Value = parse_body(resp).await;
    assert!(body["data"]["id"].is_string());
    assert!(body["data"].get("password_hash").is_none()); // DTO, not DB model
}
```

### HURL Tests — 100% coverage of HTTP endpoints

- Every endpoint must have at least one HURL test in `tests/hurl/`.
- HURL tests are the source of truth for the HTTP contract (status codes, headers, response shape).
- Name files after the resource: `users.hurl`, `orders.hurl`, `healthz.hurl`.
- Test the envelope shape explicitly.

`tests/hurl/users.hurl` — envelope-shape assertion on a wrapped endpoint:

```hurl
POST http://localhost:8080/api/users
Content-Type: application/json
{
  "email": "test@example.com",
  "password": "secret123"
}

HTTP 201
[Asserts]
jsonpath "$.data.id" isString
jsonpath "$.data.email" == "test@example.com"
jsonpath "$.data" not exists "password_hash"
```

`tests/hurl/healthz.hurl` — explicit assertion that healthz has **no** envelope:

```hurl
GET http://localhost:8080/api/healthz

HTTP 200
[Asserts]
jsonpath "$.status" == "ok"
jsonpath "$" not exists "data"
```

Two HURL requests in the same file are separated by a blank line — the file reads top-to-bottom. (Do not put a `---` separator inside a HURL file; HURL doesn't need one and it confuses markdown renderers when the snippet is embedded.)

---

## Checklist Before Committing

- [ ] Handler does not call db directly
- [ ] Service does not import HTTP types
- [ ] DB function was extended rather than duplicated where possible
- [ ] Response uses a DTO, not a DB model
- [ ] Response is wrapped in `ApiResponse` envelope (except `/api/healthz`)
- [ ] `cargo fmt` must be executed
- [ ] Unit test for every non-trivial function — handlers, services, DB functions, helpers, and any non-trivial DTO mappings or error→response conversions
- [ ] Integration test for every handler (happy + key error paths)
- [ ] HURL test for every endpoint
- [ ] No `.unwrap()` / `.expect()` in non-test code
- [ ] `AppError` handles all error cases; no ad-hoc `StatusCode` returns in handlers
