## Context

The block-character UI shipped (in the now-archived `add-server-admin-and-block-list`) with a raw EVE-character-ID number field. That is unusable: admins know griefers by name, not by 90-million-range integer. This change replaces ID entry with name search.

Two facts shape the design:

1. **The block list deliberately supports pre-emptive blocking of never-seen pilots** (the `blocked_eve_character` row is a self-contained snapshot, no FK to `eve_character`). So name search cannot be local-only — a never-seen griefer is, by definition, absent from `eve_character`. The authority on EVE names is ESI.
2. **ESI's character name search is authenticated.** `GET /characters/{character_id}/search/?categories=character&search=<q>&strict=false` requires the `esi-search.search_structures.v1` scope and a character access token, plus the `X-Compatibility-Date` header. The SSO flow already requests that scope (`handlers/auth.rs` `ESI_SCOPES`), so any admin who has logged in since carries a token that *should* work — but tokens expire and this codebase has **never made an authenticated outbound ESI call or refreshed a token before**. Every existing ESI call (`esi/public_info.rs`) is unauthenticated public-info.

The response from ESI's character search is **just an array of character IDs** — names must be resolved separately (public-info, which `public_info.rs` already does).

This change was scoped after archiving the admin/block change; `server-administration` is now a baseline this change MODIFIES.

## Goals / Non-Goals

**Goals:**

- Let an admin block a character chosen by **name**, never by raw ID.
- Search the **local index first** (fast, no external dependency, covers already-seen pilots), and only reach out to **ESI on explicit opt-in** when the local search comes up empty (covers never-seen griefers — the pre-emptive-block case).
- Make ESI search **degrade gracefully**: a missing scope, an expired/unrefreshable token, or ESI being down must yield a clear "ESI search unavailable" notice, never a 5xx and never a silent empty result that looks like "no such pilot".
- Enrich the **confirmation** with the selected character's corporation (fetched on select) so the admin can tell two same-named pilots apart before committing a block.
- Reuse the existing block endpoint (`POST /api/v1/admin/blocks { eve_character_id, reason }`) unchanged — only how the admin *arrives* at the `eve_character_id` changes.

**Non-Goals:**

- A general-purpose authenticated ESI proxy or token-refresh subsystem beyond what this one search needs. (A minimal best-effort refresh is introduced; a full refresh scheduler is out of scope.)
- Searching ESI categories other than `character`.
- Surfacing ESI search anywhere but the block-character picker.
- Changing block semantics, enforcement, or the block list's snapshot model.
- Merging local + ESI results into one ranked list. The two are sequential (local first, ESI on demand), not merged.

## Decisions

### Decision: Local-DB-first, ESI on explicit opt-in (not merged, not ESI-first)

The picker queries `GET /api/v1/admin/characters/search` (local) on every keystroke-batch ≥ 3 chars. ESI is **not** called automatically. Only when the admin sees "not found locally" do they click "search ESI", which calls `GET /api/v1/admin/characters/esi-search`. Rationale:

- The common case (block a pilot the instance already knows) needs no external call and no token — it always works, even if the admin's token is dead.
- ESI search is authenticated, rate-limited, and an-hour-cached; firing it on every keystroke is wasteful and couples ordinary blocking to the admin's token health. Opt-in keeps the fast path fast and the fragile path explicit.
- Merging the two sources would force a dedupe-by-id and a confusing mixed-provenance list. Sequential is simpler and the provenance ("found locally" vs "found via ESI") is meaningful to the admin.

Rejected: ESI-first (breaks when the token is dead, which is common); auto-merge (dedupe complexity + mixed provenance for no real gain).

### Decision: Graceful degradation is modelled as a third outcome, not an error or an empty list

The ESI search service returns a domain type that distinguishes **three** states: `Available(results)` (search ran, here are the matches — possibly empty), and `Unavailable(reason)` (search could not run: no token / refresh failed / missing scope / ESI down). The handler maps `Unavailable` to an HTTP `200` with `{ data: [], unavailable: true }` (or a typed reason), **not** a 5xx and **not** a bare empty `200` that is indistinguishable from "no matches".

Why a 200 with an indicator rather than an error status: the block flow must stay usable. An admin whose token lacks the scope can still block every already-seen pilot via local search; only the ESI fallback is degraded. Surfacing that as a 4xx/5xx would make the page feel broken. The UI reads the `unavailable` indicator and shows "ESI search unavailable — re-authorise your character". (User chose graceful-notice over surfacing the raw error.)

Rejected: propagate the ESI/token failure as the endpoint's status code (makes the whole page look broken for a degraded *fallback*); empty-200 with no indicator (the admin can't tell "no such pilot" from "search is broken").

### Decision: First authenticated ESI call — minimal, best-effort token use, transient decryption

This is the codebase's first authenticated outbound ESI call. Scope is kept minimal:

- The service resolves the admin's **main character**, reads its stored (encrypted) tokens, and decrypts the access token **transiently** for the one outbound call. The token never enters a DTO or response.
- If the access token is **expired**, the service attempts a **best-effort refresh** via the stored refresh token (the standard EVE SSO refresh-grant), persisting the new tokens. If refresh fails (no refresh token, ESF rejects it), the search resolves to `Unavailable` — it does not error.
- The `X-Compatibility-Date` header (a fixed, documented date constant) is sent on the search call. A small authenticated-ESI helper is added (`esi/search.rs` or similar) rather than overloading the public-info module.

Why best-effort refresh now: searching with a stale token would 401/403 and look "unavailable" even though the pilot was refreshable. A minimal refresh makes the common "my token expired overnight" case just work. A full refresh scheduler is out of scope; this is one inline refresh attempt at search time.

Rejected: requiring the admin to manually re-auth before every ESI search (hostile UX for a token that's trivially refreshable); a full background refresh subsystem (scope creep — this change needs one call to work).

### Decision: Search results are name + portrait; corporation is fetched on select

Both search endpoints return `eve_character_id`, `name`, `portrait_url`, and `already_blocked`. `portrait_url` is a **deterministic** image URL derived from the character id (no ESI call). Corporation is **not** in the list — it would cost one public-info call per result (N calls per search, rate-limited). Instead, when the admin **selects** a result, a single public-info fetch resolves that one character's corporation for the confirmation dialog ("Block *Wasp 223* of *Some Corp*?"). This gives disambiguation exactly where it's needed (one pilot, about to be blocked) at one call, not N.

`already_blocked` lets the picker visibly mark / disable pilots already on the list, avoiding a confusing idempotent re-block.

Rejected: corp in every result row (N ESI calls per search); no corp at all (can't disambiguate two "Wasp" pilots at the moment of blocking).

### Decision: ESI search endpoint searches as the admin's own main, and is admin-gated like the rest

`GET /api/v1/admin/characters/esi-search` takes `AdminAccount` (cookie-only), and searches on behalf of **that admin's main character's** token. It is registered in `registered_admin_routes()` so the existing fail-closed admin-coverage test enforces its gating (401/403). The 3-char minimum is enforced in the handler (400 below 3), mirroring ESI's own constraint, before any token/ESI work.

## Risks / Trade-offs

- **[Admin token lacks the search scope]** An admin who logged in before the scope was added (or declined it) can't ESI-search. → Mitigated by graceful degradation: local search still works; the notice tells them to re-authorise. Not a hard failure.
- **[First authed ESI call — refresh correctness]** Token refresh is new code with security weight (decrypt, refresh-grant, re-encrypt, persist). → Mitigated by keeping it best-effort and transient, reusing the existing encryption/token-column machinery, and unit-testing the expired→refresh→search and unrefreshable→unavailable paths. The token never leaves the server.
- **[ESI rate limits / latency on the fallback]** ESI search is hour-cached and rate-limited. → Mitigated by opt-in (not per-keystroke) and the 3-char floor; the common path never touches ESI.
- **[Corp-on-select adds a call at confirm time]** One public-info fetch when a result is selected. → Accepted: it's one call for the one pilot about to be blocked, and the block itself already best-effort-fetches a corp snapshot, so the cost profile is unchanged.
- **[Name collisions]** ESI can return several characters sharing a name fragment. → Mitigated by portrait in the list and corp in the confirmation; the block keys on the immutable id the admin selected, so the *right* id is blocked even if names collide.
- **[Compatibility-date drift]** ESI's `X-Compatibility-Date` pins a contract date; CCP advances it over time. → Accepted: a single constant to bump; documented next to the search helper.

## Open Questions

None blocking. The token-refresh helper's exact placement (a new `esi/` helper vs. extending `services/auth.rs`) is an implementation detail settled under the `rust-rest-api` skill during apply.
