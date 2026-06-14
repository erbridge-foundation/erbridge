# E-R Bridge — Architecture & Orientation

Fast orientation for agents (and humans) starting work. This is a **map, not a spec**:
it says *where* things live and *how the layers relate*, so exploration doesn't start from
`git grep` every time. Behaviour is defined by the specs in `openspec/specs/`; coding
conventions are defined by the skills. When this doc disagrees with a skill, **the skill wins**
(see `CLAUDE.md` → "Skill authority").

> **Keeping this current is mandatory.** See [Maintenance](#maintenance) at the bottom —
> every OpenSpec change that moves the structural facts here must update this file as part
> of its tasks, in the same change.

## What this is

EVE Online wormhole-mapping tool for a small known userbase (corp/alliance scale — tens of
concurrent users, a handful of admins). Engineer proportionally; see `CLAUDE.md` →
"Scale and proportionality".

- **Backend:** Rust / Axum / sqlx (Postgres). ~19.5k LOC under `backend/src`.
- **Frontend:** SvelteKit (node adapter) + Svelte 5 runes, native CSS, paraglide i18n
  (en/de/fr). ~14.7k LOC under `frontend/src`.
- **Spec-driven:** proposals/specs/tasks under `openspec/`.

Authoritative coding conventions:
- Backend: `.claude/skills/rust-rest-api/SKILL.md`
- Frontend: `.claude/skills/sveltekit-node/SKILL.md`

---

## Backend (`backend/src`)

**Layered: handler → service → db.** Handlers do HTTP (extract, authorize, shape DTOs);
services hold business logic; db holds sqlx queries. Layering is enforced by
`backend/tests/layering.rs`. DTOs and the response envelope are shared.

### Entry & wiring
- `main.rs` — binary entry; builds state, runs migrations, serves.
- `lib.rs` — **router wiring** (`/api/v1` + `/api/v1/admin` nests, `/auth/*`, `/api/health`,
  Swagger at `/api/docs`). Start here to find which handler serves a route.
- `app_state.rs` — shared `AppState` (db pool, config, ESI client, crypto keys).
- `config.rs` — env-driven configuration.
- `openapi.rs` — utoipa `ApiDoc`; kept strict by `tests/openapi_strict.rs`.

### Cross-cutting
- `error.rs` — error type → HTTP mapping. `response.rs` — the response envelope.
- `permissions.rs` — permission/role logic. `session.rs` — session model.
- `crypto.rs` — token encryption + HKDF session-key separation. `api_key.rs` — API key model.
- `audit/mod.rs` — audit-event kinds + emit helpers (kind-string house style).

### Handlers (`handlers/`)
- `auth.rs` — EVE SSO: `login`, `callback`, `logout` (POST), `add_character`.
- `middleware.rs`, `cookie.rs` — extractors/cookies. **Auth is per-handler** via the
  `AuthenticatedAccount` / `AdminAccount` extractors, **not** router-tree middleware
  (fail-closed; covered by an auth-coverage test).
- `health.rs` — `/api/health`.
- `api/v1/` — one module per resource: `me`, `account`, `characters`, `keys`, `acls`,
  `maps`, `entities`, `preferences`, `admin`.

### Services (`services/`)
`account`, `acl`, `admin`, `api_keys`, `auth`, `entity_search`, `eve_system_sync`,
`health`, `map`, `preferences`, `token_sweep` (daily ESI token-refresh sweep / token_status).
- `eve_system_sync` — daily background catalog refresh (`spawn`/`run_once`, modelled on
  `token_sweep`): fetch-all-then-write of `eve_system` / `wormhole_type` / `system_static`
  from eve-scout (`/systems`, `/wormholetypes`) + anoikis (`wh-statics`), merged on
  J-code = `eve_system.name`, with a pre-write sanity floor and a single atomic upsert txn.
- `auth` — SSO completion (`complete_sso_callback`). The bind decision consults the
  SSO `owner` hash: a presented hash differing from a non-null stored hash is a
  CCP-confirmed **transfer**, so the character is detached from the seller and
  rebound to the authenticating owner (login: fresh account; add-char: session
  account); the emptied seller becomes `account.status = 'orphaned'`.
- `admin` — also owns the irreversible **hard-delete** (`DELETE FROM account` behind
  `AdminAccount` + last-admin guard + blast-radius preview), distinct from the
  user-facing soft-delete.

### DB (`db/`)
`accounts`, `acl`, `acl_member`, `api_keys`, `blocks`, `characters`, `eve_system`, `map`,
`map_acl`, `preferences`, `sessions`. `test_helpers.rs` for test fixtures. Schema lives in
`backend/migrations/*.sql` (sequential; sqlx offline cache in `.sqlx/` must be regenerated
and committed when queries change).
- `account.status` ∈ {`active`, `soft_deleted`, `orphaned`} (CHECK-enforced).
  `orphaned` = zero characters, unreachable, never login-reactivated (distinct from
  owner-recoverable `soft_deleted`). `account.last_known_main_character_{id,name}` is a
  denormalized identity snapshot (NOT an FK), written in-tx at every `is_main` flip so an
  emptied account stays nameable. FK fallout on account delete: `eve_character`/`session`/
  `api_key` CASCADE; `map`/`acl` owner + `audit_log` actor + `blocked_eve_character`
  blocker SET NULL.
- `eve_system` — the EVE reference catalog: `eve_system` (system spine, PK `system_id`,
  `name` indexed; J-code = `name`), `wormhole_type` (type dictionary, PK `identifier`),
  `system_static` (join `(system_id, static_code)` → both FKs). Free-text `class` /
  `target_system_class` (no enum). Written only by the `eve_system_sync` service.

### ESI (`esi/`)
EVE Swagger Interface client: `token` (OAuth tokens + refresh), `jwt` + `jwks` (ESI JWT
signature verification vs SSO JWKS — note JWKS is **mixed-type**, skip non-RSA keys),
`public_info`, `search`, `rate_limit` (outbound dual-limiter), `test_support`,
`eve_scout` (typed fetches for the system-catalog sources — eve-scout `/systems` +
`/wormholetypes`, anoikis `wh-statics` with its required non-default User-Agent).

### DTOs (`dto/`)
`account`, `acl`, `admin`, `entity`, `health`, `keys`, `map`, `preferences`.

### Tests (`backend/tests/`)
Integration tests per area (`admin`, `auth`, `maps_acls`, …) plus `layering.rs`,
`openapi_strict.rs`, and **live HURL** suites under `tests/hurl/*.hurl`.

---

## Frontend (`frontend/src`)

SvelteKit pages + load functions + form actions + server endpoints. Svelte 5 runes,
native CSS with the design-token system, paraglide i18n. Load functions **forward cookies**
to the backend.

### Routes (`routes/`)
- Root `+layout.*`, `+page.*` — shell + landing.
- `account/`, `characters/`, `preferences/`, `about/`, `login/`, `blocked/` — user-facing.
- `acls/` + `acls/[id]/` — ACL list + detail (MemberPicker-driven).
- `maps/` + `maps/[slug]/` + `maps/[slug]/settings/` — map list / detail / settings.
- `admin/` (own `+layout.server.ts` gate) — `admins`, `audit` (+ `audit/more` endpoint),
  `blocks`, `characters` (client-side account datagrid).
- Endpoints: `preferences/+server.ts`, `admin/audit/more/+server.ts`.

### Shared lib (`lib/`)
- `api.ts` — backend fetch wrapper. `form-errors.ts` — form-action error shaping.
- `acl-permissions.ts`, `audit.ts` — domain helpers.
- `preferences/` — `schema`, `store.svelte`, `apply` (preference application).
- `server/env.ts` — server-only env access.
- `components/` — `GlobalNav`, `UserMenu`, `UserChip`, `Modal`, `ConfirmDialog`,
  `MemberPicker`, `PreferenceControl`, `UpdateBanner`, `AuditDetailsDialog`.
- `paraglide/` — generated i18n (compiled from messages; **run scripts from `frontend/`**,
  not `pnpm --filter`).

### Verifying frontend changes
From `frontend/`: `pnpm test` (Vitest), `pnpm run check` (svelte-check + paraglide),
`pnpm run test:e2e` (Playwright). All three required — see `CLAUDE.md`.

---

## OpenSpec workflow (`openspec/`)

- `specs/` — current capability specs (authoritative behaviour).
- `changes/` — in-flight proposals (`proposal.md` + `design.md` + `tasks.md` + delta specs);
  `changes/archive/` holds completed ones.
- `config.yaml` — openspec config.
- Skills `openspec-explore` / `-propose` / `-apply-change` / `-sync-specs` / `-archive-change`
  drive the lifecycle.

---

## Maintenance

**This file MUST be kept in sync with the codebase, as part of the OpenSpec change that
changes it.** A change touches this doc when it does any of:

- adds, removes, renames, or **relocates** a module/route/service/db/dto/component
  (i.e. any of the trees above goes stale);
- changes a structural fact stated here (layering, auth model, router wiring, ESI/crypto
  notes, the i18n/locale or verification commands).

When generating a change's `tasks.md`, if the change does any of the above, the tasks MUST
include an explicit step to update `openspec/AGENTS.md` in the **same change** (alongside
spec deltas — before the change is marked complete). Pure-behaviour changes that don't move
any structural fact don't need to touch this file.

Keep edits a *map*, not a changelog: state where things are now; don't accrete history.
