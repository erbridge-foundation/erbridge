# Align root docs with reality and gate future doc drift

## Why

An audit of the repo's root `.md` files against the code found that most are
accurate (`CLAUDE.md`, root `AGENTS.md`, `CONTRIBUTING.md`, `RELEASING.md`,
`backend/README.md`), but three concrete drifts have accumulated in the
human-facing docs:

1. **`README.md` documents `ESI_CALLBACK_URL` as a config variable, but the
   backend never reads it.** `config.rs` has no such field, `.env.example` does
   not list it, and the OAuth callback URL is hardcoded as
   `{APP_URL}/auth/callback` at three callsites. A deployer behind a proxy who
   sets the documented override would see it silently ignored and hit an OAuth
   redirect-URI mismatch.

2. **`README.md` says the hurl integration tests live in `hurl/` at the repo
   root.** They actually live in `backend/tests/hurl/`; no root `hurl/` exists.

3. **`frontend/README.md` is the untouched `sv create` scaffold** (title `# sv`,
   tells the reader to run `npx sv create` and `npm run dev`). It describes none
   of the shipped frontend and actively contradicts the repo's pnpm-only rule.

The deeper problem is that there is **no standing rule keeping the root docs in
sync**, the way the "Architecture doc upkeep" rule keeps `openspec/AGENTS.md`
current in-change. That gap is exactly how these three drifts rotted unnoticed.

## What Changes

- **Implement `ESI_CALLBACK_URL`** (chosen over deleting it from the README):
  add it to `Config` and `.env.example` as an OPTIONAL variable defaulting to
  `{APP_URL}/auth/callback`, and derive the callback URL once so all three
  callsites (login, add-character, token exchange) use the configured value.
  This makes the README true and gives proxied deployments a real escape hatch.
- **Fix the `README.md` hurl path** to `backend/tests/hurl/`.
- **Rewrite `frontend/README.md`** from the `sv` stub into a real doc: pnpm-only
  commands, the three verification commands (`pnpm test`, `pnpm run check`,
  `pnpm run test:e2e`), a pointer to the `sveltekit-node` skill and
  `openspec/AGENTS.md`, and the actual route surface.
- **Add a "Root-doc upkeep" rule to `CLAUDE.md`**, mirroring the existing
  "Architecture doc upkeep" rule: a change that touches a fact the root docs
  describe (config/env vars, deploy/release flow, setup steps, route mounts,
  documented commands) MUST reconcile the affected root doc **in the same
  change**, and the change's `tasks.md` MUST list that step. Doc-only and
  review-enforced — no CI machinery, matching the scale and the precedent of the
  AGENTS.md rule.

## Impact

- Affected specs: `project-infrastructure` (env-var requirement gains the
  optional `ESI_CALLBACK_URL`), `eve-sso-auth` (redirect-URI requirement now
  derives from the configured callback URL).
- Affected code: `backend/src/config.rs`, `backend/.env.example`,
  `backend/src/handlers/auth.rs` (three callsites).
- Affected docs: `README.md`, `frontend/README.md`, `CLAUDE.md`.
- No migration. No frontend code change (frontend README is docs only).
- Process impact: every future change now carries a doc-reconciliation
  obligation when it moves a documented fact — the recurrence gate the audit
  asked for.
