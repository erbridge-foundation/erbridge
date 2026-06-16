# Tasks — Align root docs and gate drift

## 1. Implement `ESI_CALLBACK_URL` (backend)

- [x] 1.1 Add an `esi_callback_url: String` field to `Config` in
      `backend/src/config.rs`, resolved in `from_env()` as
      `std::env::var("ESI_CALLBACK_URL")` if set, else
      `format!("{}/auth/callback", app_url)`. (Read the `rust-rest-api` skill
      before touching backend code.)
- [x] 1.2 Replace the three hardcoded `{app_url}/auth/callback` constructions in
      `backend/src/handlers/auth.rs` (login redirect, token-exchange,
      add-character redirect) with reads of `state.config.esi_callback_url`.
- [x] 1.3 Add `ESI_CALLBACK_URL` to `backend/.env.example`, commented as OPTIONAL
      with its `{APP_URL}/auth/callback` default documented.
- [x] 1.4 Add/extend a unit test asserting: unset → `{app_url}/auth/callback`;
      set → exact value used. Update any auth handler test fixtures that build
      `Config` to populate the new field.
- [x] 1.5 If the live hurl auth flow asserts the `redirect_uri`, confirm it still
      passes with the default-path behaviour.

## 2. Fix `README.md`

- [x] 2.1 Keep the `ESI_CALLBACK_URL` row in the config table now that it is real;
      confirm its description matches the implemented default.
- [x] 2.2 Correct the hurl path: `hurl/` → `backend/tests/hurl/`.

## 3. Rewrite `frontend/README.md`

- [x] 3.1 Replace the stock `sv` scaffold content with: project one-liner,
      pnpm-only dev/build commands, the three verification commands
      (`pnpm test`, `pnpm run check`, `pnpm run test:e2e`, run from `frontend/`),
      and pointers to the `sveltekit-node` skill and `openspec/AGENTS.md` as the
      authoritative structure sources. No npm/npx anywhere.

## 4. Add the "Root-doc upkeep" rule to `CLAUDE.md`

- [x] 4.1 Insert the new section (wording in `design.md`) adjacent to the
      "Architecture doc upkeep" section.

## 5. Architecture doc check

- [x] 5.1 Confirm whether `openspec/AGENTS.md` states the callback URL or env-var
      facts; if it does, update them. (No structural module/route change here, so
      likely no edit needed — verify, don't assume.)

## 6. Verification

- [x] 6.1 Backend: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo sqlx prepare --check -- --all-targets`, `cargo test --all-targets`
      (from `backend/`).
- [x] 6.2 No frontend *code* changed (frontend README is docs only), so the
      frontend test trio is not required by this change — note this explicitly in
      the PR so it is a deliberate skip, not an omission.
- [x] 6.3 `openspec validate align-root-docs-and-gate-drift --strict`.
