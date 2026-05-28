## Context

Soft-delete today is a status flip on `account`: `status = 'soft_deleted'`, `delete_requested_at = now()`. Linked `eve_character` rows are untouched, which leaves `encrypted_access_token` and `encrypted_refresh_token` on disk — encrypted at rest with `encryption_secret`, but still functional once decrypted — for the entire soft-delete window (until hard-purge runs).

The `account-management` spec currently says "Character rows SHALL NOT be modified" in the soft-delete requirement. That wording was written to communicate "the row keeps existing so reactivation can find it"; it has been (correctly) interpreted by the implementation as "don't touch their columns either." This change resolves the ambiguity in favour of clearing tokens.

## Goals / Non-Goals

**Goals**
- Spec states explicitly that soft-deleting an account clears the EVE token material for every linked character, in the same transaction as the status flip.
- The rule is phrased around the soft-delete *event* (the `status` transition), not the actor that triggered it, so it applies uniformly to the existing owner-initiated endpoint and any future admin- or system-initiated path.
- Reactivation behaviour is documented: re-login restores `status = 'active'` and writes fresh tokens for the character that logged in; other linked characters come back with `token_status = "expired"` and must be re-SSO'd individually.
- The spec is honest about the limit of the guarantee: this clears our copy, it does not revoke EVE-side app authorisation.

**Non-Goals**
- Calling EVE's SSO token-revocation endpoint during soft-delete. The user can revoke at EVE SSO themselves; server-side revocation is a separate change if we want it.
- Changing hard-delete (the eventual purge job) — that path already removes the rows entirely.
- Changing how `token_status` is derived. It still comes from `encrypted_refresh_token IS NOT NULL`; clearing the column flips it to `"expired"` automatically.
- Reworking the reactivation UX (e.g., banner saying "re-auth your alts"). The per-character `token_status` already drives the existing affordances; this change does not add new UI.

## Decisions

### Decision: Bundle the session-cookie suppression fix into this change

Verification with hurl uncovered that `refresh_session_cookie` middleware was overwriting `delete_account`'s cookie-clearing `Set-Cookie` with a freshly-minted session cookie. The browser stayed logged in across the soft-delete despite the handler's intent. Both files predate this change, so the bug is older — but it's the same "delete = gone" mental model this change exists to honour, and the spec's existing prose ("the response clears the session cookie") was already a lie. Splitting into a second change would force the spec wording to either stay-a-lie temporarily or be edited twice. Folding the fix here is the lower-friction call.

Rejected alternative: open a separate change. It would have the same code, an almost-identical spec delta, and would have to land before this change can claim its spec promise is honest.

### Decision: Handler explicitly suppresses the refresh slot, instead of middleware sniffing the response

Three approaches considered:

- **A.** Middleware checks the response for an existing cookie-clearing `Set-Cookie` and skips the refresh write when one is present. Implicit; works for any future handler with no coordination. Rejected because the middleware would have to parse `Set-Cookie` headers and recognise the clearing pattern — fragile, and couples the middleware to a textual format.
- **B.** A side-effect extractor (`SuppressSessionRefresh`) added to the handler signature. Rejected as too clever — extractor ordering relative to `AuthenticatedAccount` matters (the extractor must run *after* auth, otherwise the slot gets refilled), and the side-effect-only extractor pattern is unusual in axum.
- **C.** The handler explicitly pulls `Extension<RefreshedJwtSlot>` and calls `.suppress()` after its work succeeds. Three lines in the handler; reuses the existing slot abstraction. The action is named, deliberate, and visible at the place the contract is established ("this endpoint logs the user out").

Picked C. Trade: `RefreshedJwtSlot` becomes `pub` so handlers can name the extension type, and a `pub fn suppress(&self)` is added for clarity over reaching into the inner `Mutex`. Surface area increase is minimal and the type was already accessible via the request extensions slot.

### Decision: Clear refresh + access tokens + expiry + scopes, not just refresh

Rationale: leaving `scopes` and `access_token_expires_at` populated while the token columns are NULL is a meaningless half-state. The four columns travel as a unit at write time (the upsert path in `db/characters.rs` writes them together); they should also travel as a unit on clear. `scopes` is reset to an empty array rather than NULL because the column is `NOT NULL` per the schema.

Rejected alternative: clear only `encrypted_access_token`. This was floated in the original stub as a "middle ground" because `token_status` derives from refresh-token presence, but it buys roughly nothing — the access token has a ~20 minute TTL and is regenerated from the refresh token on the next ESI call, so an attacker with the refresh token is not slowed down. It would also leave the spec explaining why the half-measure is meaningful.

### Decision: Phrase the rule around the soft-delete event, not the actor

Today only `DELETE /api/v1/account` (caller-initiated) reaches `soft_delete`. The spec deliberately does not enumerate actors — the rule is "whenever an `account` row transitions to `status = 'soft_deleted'`, the linked characters' token columns are cleared in the same transaction." Any future admin-initiated delete, scheduled-inactivity sweep, or compliance-driven purge inherits the rule for free.

Rejected alternative: spell out "owner-initiated soft-delete" explicitly. This invites a second requirement when we add an admin path, and risks the admin path silently skipping the clear.

### Decision: Atomic with the status flip, not a follow-up step

`soft_delete` is being changed from taking `&PgPool` to `&mut Transaction<'_, Postgres>`, so the service layer can call `soft_delete` and `clear_tokens_for_account` in one transaction and commit at the end. A partial failure that flipped status without clearing tokens would leave the system in exactly the state this change exists to prevent.

Rejected alternative: two-phase with a background sweeper that catches stragglers. Adds moving parts (a job, a state to represent "status flipped, clear pending"), and the all-or-nothing transactional version is straightforward — both writes target the same database and there is no external IO between them.

### Decision: No EVE-side revocation in this change

We do not call EVE's token revocation endpoint. Adding it means: an extra outbound dependency on the soft-delete path (what if EVE SSO is down?), a decision about whether revocation failure should fail the soft-delete (probably no, since the user expects the delete to succeed), and per-character HTTP fan-out inside the request. None of those are deal-breakers, but they are scope of their own. The spec notes the gap and points users at EVE SSO; a future change can layer revocation on top without re-litigating the column-clearing rule.

### Decision: Reactivation does not auto-refresh non-logged-in characters

When a soft-deleted account reactivates via SSO, only the character that just logged in gets fresh tokens (via the existing `upsert_character_tokens` path). Other linked alts come back with `encrypted_refresh_token = NULL` → `token_status = "expired"` and the user re-SSOs them from the account page using the existing "+ add character" / re-auth flow.

Rejected alternative: block reactivation until the user re-auths every character. Heavy-handed; the user can use the account immediately with one character and re-auth the rest at leisure, which matches how the UI already handles expired tokens.

## Risks / Trade-offs

- **Reactivation friction is the cost we are accepting.** A user with N linked characters who soft-deletes and changes their mind has to walk through SSO N times (one to reactivate + N-1 for the alts). The mitigation is that this is the same flow they already use for ordinary token expiry, and the typical "I clicked the wrong button" recovery happens before they have many alts at risk.
- **The spec promises less than users might assume.** Soft-delete clears our copy of the refresh token; it does not invalidate it at EVE SSO. An attacker who already had the plaintext refresh token before the soft-delete is unaffected — but that's a strict superset of what the status quo gave us. The spec is written to be honest about this.
- **Signature change in the db layer.** `accounts::soft_delete` moves from `&PgPool` to `&mut Transaction`. Callers update accordingly; the only caller today is `services/account.rs::delete_account`. The existing sqlx test `soft_delete_sets_status` updates to open a transaction.
- **No new external dependency.** Pure in-database work, atomic, single transaction.

## Migration Plan

No data migration. Existing soft-deleted accounts (if any) keep their token columns until hard-purge runs; the new behaviour applies to soft-deletes performed after the change ships. The product owner can choose to run a one-off backfill (`UPDATE eve_character SET encrypted_access_token = NULL, encrypted_refresh_token = NULL, access_token_expires_at = NULL, scopes = '{}' WHERE account_id IN (SELECT id FROM account WHERE status = 'soft_deleted')`) if there are pre-existing soft-deleted accounts on disk; that backfill is out of scope here.

## Open Questions

None. Server-side EVE revocation is deferred to a future change by design, not by ambiguity.
