## Why

The block-character UI (shipped in `add-server-admin-and-block-list` §9) makes an admin type a raw EVE character ID into a number field. Nobody knows character IDs; an admin who wants to block a griefer they know by *name* has no way to find the ID without an external tool. The block list keys on the immutable character ID precisely so unknown pilots can be pre-emptively blocked — but the *UI* for choosing whom to block must let an admin search by name.

A name search has two natural sources. Characters already seen by this instance live in the local `eve_character` table (the existing `GET /api/v1/admin/characters/search` already does a substring match there). But a never-seen griefer — the exact case pre-emptive blocking exists for — is not in the local table, so the admin must reach out to EVE's authority on names: ESI. ESI's `GET /characters/{character_id}/search/` does an authenticated case-insensitive substring search (`categories=character`, `search` ≥ 3 chars), returning character IDs the app then resolves to names. This is the first authenticated ESI call in the codebase; the SSO flow already requests the required `esi-search.search_structures.v1` scope, so admins who have logged in since carry a usable token.

## What Changes

- **NEW** `esi-search` capability: an authenticated ESI character-name search performed on behalf of a given character. The backend decrypts the searching admin's stored access token, calls `GET /characters/{admin_character_id}/search/?categories=character&search=<q>&strict=false` (with the required `X-Compatibility-Date` header), and resolves the returned IDs to `(eve_character_id, name)` via public-info, yielding ready-to-display results (name + deterministic portrait URL). The search SHALL require `q` of at least 3 characters (matching ESI's own minimum).

- **MODIFIED** `server-administration`:
  - **NEW** `GET /api/v1/admin/characters/esi-search?q=<fragment>` — the ESI-backed fallback. Gated by `AdminAccount` like every admin route. It searches ESI on behalf of the admin's **own main character**. It SHALL require `q` ≥ 3 characters (400 otherwise). When the admin's token cannot perform the search (missing scope, expired/unrefreshable, or ESI unavailable), the endpoint SHALL respond gracefully with an empty result and a machine-readable `esi_search_unavailable` reason rather than a 5xx, so the UI can show "ESI search unavailable — re-authorise your character" without the block flow breaking. Each result carries `eve_character_id`, `name`, `portrait_url`, and `already_blocked` (so the UI can mark pilots already on the list).
  - **MODIFIED** the existing `GET /api/v1/admin/characters/search` (local DB) result shape gains `portrait_url` and `already_blocked` so the two search sources render identically. The substring/case-insensitive/cap behaviour is unchanged.
  - **MODIFIED** the block flow: `POST /api/v1/admin/blocks` is unchanged on the wire (still `{ eve_character_id, reason }`), but the corp snapshot it already fetches best-effort stays as-is. No raw-ID entry remains in the supported UI contract.

- **MODIFIED** frontend `/admin/blocks`: the "Block a character" form drops the raw **EVE character ID** number field. In its place: a name search (min 3 chars) that queries the **local DB first**; if the pilot isn't found there, the admin can **opt in to an ESI search** ("not in the local list — search ESI"). Selecting a result shows a confirmation enriched with the character's **corporation** (fetched on select for extra clarity) before the block is submitted with the resolved `eve_character_id` and an optional reason. A clear notice is shown if ESI search is unavailable.

## Capabilities

### New Capabilities

- `esi-search`: authenticated ESI character-name search on behalf of a character — token decryption + best-effort refresh, the `GET /characters/{character_id}/search/` call with the compatibility-date header and `esi-search.search_structures.v1` scope, ID-to-name resolution, and graceful degradation when the token/scope/ESI is unavailable.

### Modified Capabilities

- `server-administration`: adds the ESI-backed character-search admin endpoint; enriches both character-search result shapes with `portrait_url` + `already_blocked`; redefines the block-character UI contract around name search (DB-first, ESI fallback, corp-on-select) and removes raw character-ID entry.

## Impact

- **Backend** (per `rust-rest-api`):
  - New `esi/search.rs` (or extend `esi/`): the authenticated `character_search(http, admin_character_id, access_token, q) -> Result<Vec<i64>>` call (compat-date header, `categories=character`, `strict=false`), and ID→name resolution (reuse/extend `public_info`). This is the **first authenticated outbound ESI call** — it needs the admin character's decrypted access token; a token-decrypt + best-effort-refresh helper is introduced (no refresh path exists today).
  - New `services/admin.rs::esi_search_characters` orchestrating: resolve the admin's main character → decrypt token → ESI search → resolve names → annotate `already_blocked` via the block list. Returns a domain result that distinguishes "no matches" from "search unavailable".
  - New handler `GET /api/v1/admin/characters/esi-search` in `handlers/api/v1/admin.rs`; registered in `lib.rs` + `registered_admin_routes()` (so the fail-closed admin-coverage test covers it) + `openapi.rs`.
  - DTO changes in `dto/admin.rs`: `portrait_url` + `already_blocked` on the character-search result DTO; a new `EsiCharacterSearchResultDto` / page wrapper carrying the unavailable reason.
  - `.sqlx/` cache regenerated if any query changes.
  - Full unit + integration + HURL coverage: ESI search happy path, `q` < 3 → 400, token-missing-scope → graceful empty, ESI-down → graceful empty, `already_blocked` annotation, admin-gating (401/403/bearer-401).

- **Frontend** (per `sveltekit-node`): `/admin/blocks/+page.svelte` + `+page.server.ts` reworked — remove the ID field, add the DB-first/ESI-fallback name picker (a `search` form action for DB, a second for ESI), corp-on-select confirmation, and the unavailable notice. New i18n keys (en + de). Vitest (search actions, min-length, ESI-unavailable branch, DB→ESI fallthrough) + Playwright (search→select→confirm→block; ESI-unavailable notice). All three frontend gates required.

- **Out of scope**: refreshing tokens for non-search purposes; a general-purpose ESI proxy; searching categories other than `character`; surfacing ESI search anywhere but the block UI.
