## Why

The `rust-rest-api` skill defines the backend's layering, error handling, and DTO rules, but compliance is voluntary — Claude sessions have repeatedly violated those rules (handler→service→db inversion, DTOs depending on service types, string-matching SQL error messages for conflict detection, missing test scaffolding). The skill itself flags this: *"If none of these are in place, the architecture rule is aspirational only."* The foundation change has now landed enough code that the cost of drift compounds with every new handler; we need a gate that fails the build when a violation lands, not a document that asks the agent nicely. This change introduces that gate for the backend so the rules become load-bearing. A separate `frontend-enforcement-layer` change will follow when §4 begins.

## What Changes

- New `backend/clippy.toml` declaring `disallowed-types` and `disallowed-methods` entries that encode the `rust-rest-api` skill's layering rules at the compiler/linter boundary:
  - `crate::db::*` MUST NOT be imported from `crate::handlers::*` (handlers go through services).
  - `axum::*`, `axum_core::*`, and `http::*` MUST NOT be imported from `crate::services::*` (services own no HTTP types).
  - `serde::Serialize` MUST NOT be derived on a struct in `crate::db::*` (DB models are not serialisable by accident; DTOs do that).
- `cargo clippy --all-targets -- -D warnings` becomes a required check; the lint config is set up so it fails CI on a real violation but does not produce false positives on the existing codebase as of the foundation's current state.
- New `.github/workflows/backend.yml` (or equivalent runner — final venue chosen in design.md) running `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` on every push and PR that touches `backend/**`. Tests run against a Postgres service container provided by GitHub Actions (`services: postgres`); compile-time `sqlx::query!` validation uses the committed `backend/.sqlx/` offline cache (`SQLX_OFFLINE=true`).
- A deliberate-violation test: a tracked file `backend/tests/enforcement_gate.md` documents three deliberate violations (one per rule above), each accompanied by the expected clippy error. CI does NOT run these violations as code — the file is the proof-of-bite reference. When this change is implemented, the implementer SHALL temporarily introduce each violation locally, confirm clippy rejects it with the documented error, then revert. This is the §2d.6 / §7.27 pattern from the foundation change: prove the gate bites before declaring the change done.
- A README note in `backend/README.md` (or new file) pointing future contributors at `clippy.toml` as the load-bearing source of the layering rules, with a pointer back to the `rust-rest-api` skill for the rationale.

## Capabilities

### New Capabilities

- `backend-enforcement`: Lint and CI gates that mechanically enforce the `rust-rest-api` skill's layering, error-handling, and DTO rules. Encompasses `clippy.toml` configuration, CI workflow definitions, and the deliberate-violation test pattern that proves each gate bites.

### Modified Capabilities

## Impact

- New files: `backend/clippy.toml`, `.github/workflows/backend.yml` (or chosen CI venue), `backend/tests/enforcement_gate.md`, optional `backend/README.md` addendum.
- No code under `backend/src/` changes as part of this proposal. If clippy surfaces a real violation in the existing codebase, it is fixed under this change (the spec is "the gate passes on a clean codebase"); a violation that the foundation tolerates is documented and either fixed or `#[allow(...)]`'d with an explicit comment pointing at the spec task that consumes it.
- No new runtime dependencies. `clippy` ships with the Rust toolchain; the CI runner is whatever the project chooses (GitHub Actions assumed unless design.md picks otherwise).
- Future backend changes are now blocked by the gate. This is intentional and is the entire point of the change.
- The frontend tooling, build, and `frontend/` directory are out of scope. A separate `frontend-enforcement-layer` change will mirror this pattern when §4 work begins.
