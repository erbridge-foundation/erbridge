## Why

The server has exactly one way to gain administrative power today: the first account to complete SSO is auto-promoted to `is_server_admin` (the bootstrap rule). There is no way to add a second admin, no way to remove one, and no way to keep a disruptive pilot off the server. For a tool that could scale to a public instance with hundreds of users and thousands of characters, "the founder is the only admin forever, and griefers are unstoppable" is not viable.

This change adds the two first-class admin capabilities the server needs: **managing the set of server admins** (grant/revoke), and a **character block list** that bans a pilot — and, by extension, their whole account — from the server. The audit foundation (`add-audit-log`) already ships the dormant `server_admin_granted{admin_grant}`, `server_admin_revoked`, `eve_character_blocked`, and `eve_character_unblocked` variants; this change activates them.

## What Changes

- **NEW** `server-administration` capability: an `AdminAccount` extractor (session-cookie auth only — admin actions never authenticate via API key), admin-only endpoints under `/api/v1/admin/*`, the `blocked_eve_character` table, and the block-enforcement rules. Specifically:
  - **Admin management.** `GET /api/v1/admin/accounts` (list), `POST /api/v1/admin/accounts/:id/grant-admin`, `POST /api/v1/admin/accounts/:id/revoke-admin`. Revoke enforces a last-admin guard (cannot remove the final active admin) inside the transaction. Grant/revoke are idempotent.
  - **Character search** for the grant UI: `GET /api/v1/admin/characters/search?q=` resolves a name fragment to characters and their owning accounts, so an admin can promote "the account that owns *Pilot X*" at scale without listing every account.
  - **Block list.** `GET /api/v1/admin/blocks` (list), `POST /api/v1/admin/blocks` (block an EVE character ID, optional reason), `DELETE /api/v1/admin/blocks/:eve_character_id` (unblock). The block row is **self-contained**: it denormalises the character's name and corporation onto the block row (snapshot), so the list never joins to `eve_character` and pre-emptive blocks of never-seen pilots work. ESI public-info is fetched best-effort to populate the snapshot; a block SHALL succeed even when ESI is unavailable (name/corp left null).
  - **Block semantics.** Blocking an EVE character bans the whole owning account (derived: an account is blocked iff it owns any blocked character). Blocking SHALL, in one transaction: insert the block row; and *if the character currently resolves to an account*, clear that account's EVE tokens (same columns as soft-delete) and delete all its sessions. An admin SHALL NOT block any of their own characters (self-block guard).
  - **Block enforcement** mirrors the existing soft-delete model exactly. A blocked account has no live session (sessions were deleted on block) and cannot obtain a new one (the SSO callback rejects blocked characters). The only surviving auth route — `Authorization: Bearer erb_…` — gets an explicit block check in the bearer branch of `AuthenticatedAccount` (a join against the block list), right where the `soft_deleted` check already lives. The session-cookie hot path (e.g. `GET /api/v1/me` on every page load) gets **no** new check, identical to how soft-delete is handled.
  - **Admin audit browser.** `GET /api/v1/admin/audit` exposes `audit::list_audit_log` (built in `add-audit-log`, target axis added in `add-audit-log-target-columns`) with cursor pagination (`before`), `event_type` / `actor` / `target_type` / `target_id` / `target_name` filters, and a clamped `limit`. The target filters are the dominant admin query ("who did X to whom"); `target_name` is the human-searchable axis.

- **NEW** frontend `/admin` route group, server-side gated so non-admins get a 404 (the existence of admin pages is not disclosed). Pages: `/admin` (overview), `/admin/admins` (list + grant via character-search + revoke), `/admin/blocks` (list + block + unblock), `/admin/audit` (filterable, paginated log). The user menu gains an "Admin" link only when `is_server_admin`. A `/blocked` informational landing page explains the block when a blocked pilot's bearer request is rejected.

- **MODIFIED** `eve-sso-auth`: the OAuth2 callback SHALL reject a login whose resolved `eve_character_id` is in the block list — no account write, no session, no token persistence — for both the login flow and the add-character flow (so a blocked pilot cannot become someone's alt). The rejection emits an audit event.

- **MODIFIED** `api-authentication`: the bearer branch of the `AuthenticatedAccount` extractor SHALL reject a request whose account owns a blocked character (new `account_blocked` error), alongside the existing `account_soft_deleted` check.

- **MODIFIED** `account-management`: `GET /api/v1/me` already returns `is_server_admin`; document that the frontend uses it to gate the admin-menu affordance. (No behavioural change to `/me` itself.)

- **MODIFIED** `audit-log`: activate the four dormant admin/block variants and add one new variant, `blocked_login_rejected`, recording a rejected SSO attempt by a blocked character (actor null; `eve_character_id` in `details`). This is a deliberate, narrow extension of the audit log to cover a security-relevant *attempt* (not just committed state changes) — justified for a community tool where "is this blocked pilot still trying to get in?" is a real admin question.

## Capabilities

### New Capabilities

- `server-administration`: admin-role management (grant/revoke with last-admin guard), the `AdminAccount` cookie-only extractor and its fail-closed coverage test, character search for the grant UI, the `blocked_eve_character` table and block/unblock endpoints, block semantics (token-clear + session-teardown of the owning account, self-block guard), block enforcement at SSO and on the bearer route, and the admin audit-log read endpoint.

### Modified Capabilities

- `eve-sso-auth`: callback rejects blocked characters (login + add-character flows) and emits `blocked_login_rejected`.
- `api-authentication`: `AuthenticatedAccount` bearer branch rejects accounts owning a blocked character.
- `account-management`: documents that `/me`'s `is_server_admin` gates the admin UI affordance (no behavioural change).
- `audit-log`: activates `server_admin_granted{admin_grant}`, `server_admin_revoked`, `eve_character_blocked`, `eve_character_unblocked`; adds `blocked_login_rejected`.

## Impact

- **Backend**:
  - New migration `00000000000006_create_blocked_eve_character.sql`: `blocked_eve_character (eve_character_id BIGINT PRIMARY KEY, character_name TEXT, corporation_name TEXT, reason TEXT, blocked_by UUID REFERENCES account(id) ON DELETE SET NULL, blocked_at TIMESTAMPTZ NOT NULL DEFAULT now())`. **No FK** to `eve_character` — the row is a self-contained snapshot so unknown pilots can be blocked.
  - New `AdminAccount` extractor in `handlers/middleware.rs` (or a sibling), session-cookie only, resolving to `Uuid` and rejecting non-admins with 403 `forbidden_admin_required`. A coverage test asserts every `/api/v1/admin/*` route extracts `AdminAccount` (fail-closed by omission), mirroring the existing auth-coverage test.
  - New `handlers/api/v1/admin.rs`, `services/admin.rs`, `db/blocks.rs` (or extend `db/accounts.rs`) per the `rust-rest-api` layered layout. New DTOs under `dto/admin.rs`.
  - `AuthenticatedAccount` bearer branch gains a block-list join + new `AppError::AccountBlocked` (401, `account_blocked`).
  - SSO completion (`services/auth.rs::complete_sso_callback`) gains a block check before any account/character write; rejects with a blocked error and emits `blocked_login_rejected`.
  - `db/accounts.rs`: `set_server_admin`, `count_server_admins` (already exists), `account_has_blocked_character`, `is_eve_character_blocked`; block insert/delete/list; character-search query.
  - `audit/mod.rs`: add the `BlockedLoginRejected { eve_character_id }` variant (the other four already exist).
  - Full unit + integration + HURL coverage per the backend skill: extractor coverage test, last-admin guard, self-block guard, block-clears-tokens-and-kills-sessions, SSO-rejects-blocked (login + add-character), bearer-rejects-blocked, cookie-path-unaffected, idempotent grant/block, audit emissions for every admin action and the rejected-login attempt.
  - `.sqlx/` cache regenerated.

- **Frontend** (per `sveltekit-node`): `/admin` route group with `+layout.server.ts` 404-gating non-admins; `/admin`, `/admin/admins`, `/admin/blocks`, `/admin/audit` pages; `/blocked` landing; user-menu "Admin" link gated on `is_server_admin`; form actions for grant/revoke/block/unblock; the character-search-to-grant flow. Verification MUST run all three frontend gates (`pnpm --filter frontend test`, `pnpm --filter frontend run check`, `pnpm --filter frontend run test:e2e`).

- **Out of scope** (deferred, consistent with the dormant-variant catalogue):
  - Map/ACL admin overrides (`admin_map_*`, `admin_acl_*`) — those land with the wormhole-mapper features that introduce maps and ACLs.
  - Account hard-purge/restore (`account_purged`) — a future grace-period purge flow.
  - Server-scoped API keys for admin automation — admin actions are cookie-only by design in v1.
  - Time-boxed / auto-expiring blocks — v1 blocks are indefinite until an admin unblocks.
