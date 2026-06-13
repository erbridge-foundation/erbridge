## Why

EVE's SSO `owner` hash is CCP's canonical "this character changed hands" signal — it rotates when a character is sold or transferred to a different EVE account. Today the SSO bind path keys every decision on `eve_character_id` alone and never consults the hash, so a transferred character produces the wrong outcome: on **login** the new owner is silently dropped into the *seller's* ERB account, and on **add-character** the add is hard-rejected as `BoundElsewhere` even though the presenter legitimately owns the character now. The owner hash is already captured at bind time (we just discard it for binding); acting on it is the missing piece.

## What Changes

- **Detect transfer at bind time (login *and* add-character).** When a presented character's `owner_hash` is present, the stored `owner_hash` on its existing row is present, and the two **differ**, treat it as a CCP-confirmed transfer instead of a conflict: detach the character from the prior (seller's) account and bind it to the new owner — the session account in add-character mode, a freshly-resolved account in login mode.
- **Conservative fallback.** If the presented hash is absent, or the hashes **match** across accounts, keep the existing `BoundElsewhere` rejection. A matching hash on the *same* account during login remains the existing self-heal. Detaching only ever happens on present-and-differing evidence — never on an absent or unprovable hash.
- **Seller-side fixup after detach.** If the seller account still has characters but lost its main, re-promote a remaining character. If it now has **zero** characters it is unreachable (ERB identity is derived entirely from characters), so it transitions to a new terminal `status = 'orphaned'` — the row is kept, never deleted.
- **Denormalize the main onto the account** so an orphaned (zero-character) account stays nameable and findable: new columns `last_known_main_character_id` (BIGINT, **not** a foreign key) and `last_known_main_character_name` (TEXT), maintained in-transaction at every `is_main` flip.
- **Audit a character-transferred event**, actored by the destination account, snapshotting the seller account id + name into `details` (self-contained, no read-time resolution).
- **Add a narrow server-admin hard-delete-account capability.** This is the in-app resolution for the one documented dead-end this feature's conditions can create (below). It performs a real `DELETE FROM account` behind the admin extractor, reuses the last-server-admin guard, emits an audit event, and is gated behind a **deletion preview** showing the blast radius before confirmation.
- **BREAKING (data/state machine):** the `account.status` domain gains a new terminal value `'orphaned'`; consumers that enumerate statuses must account for it.

### Known limitations (documented, intentionally **not** solved here)

- A human who already has an ERB account but logs in **fresh** with a newly-transferred character receives a *second, separate* account for it — ERB has no identity link between the two accounts beyond the character itself. Resolution is a **server admin hard-deleting the spare account** (the capability this change adds), after which the human adds the character to their original account normally. (Today's user-facing delete is soft-delete only and retains characters, so without admin hard-delete this corner has no in-app exit.)
- The inverse problem — same-human, **matching**-hash re-homing (account merge / admin character-move between a human's own accounts) — is explicitly out of scope.

## Capabilities

### New Capabilities
- `account-hard-delete`: a server-admin-only irreversible account deletion (`DELETE FROM account`) with a pre-deletion blast-radius preview, the last-server-admin guard, and an audit event. Documents the cascade/SET-NULL fallout (characters/sessions/keys removed; maps/ACLs become unowned; audit history preserved).

### Modified Capabilities
- `eve-sso-auth`: the callback bind decision compares presented vs stored `owner_hash` and, on a present-and-differing pair, detaches the character from its prior account and rebinds it to the new owner (login: fresh account; add-character: session account) instead of returning the seller's account (login) or rejecting `BoundElsewhere` (add-character).
- `character-token-lifecycle`: owner-hash *change* is now also acted on at bind time (detach/rebind), complementing the daily sweep's passive `owner_mismatch` flagging.
- `account-management`: introduces the `'orphaned'` terminal status (zero-character, unreachable, distinct from owner-recoverable `soft_deleted`) and the denormalized `last_known_main_character_id` / `last_known_main_character_name` identity snapshot maintained at every `is_main` change.
- `server-administration`: gains the admin hard-delete-account action and its deletion-preview surface.

## Impact

- **Schema** (new migration): `account.last_known_main_character_id BIGINT NULL`, `account.last_known_main_character_name TEXT NULL`, and extend the `account.status` domain/CHECK to include `'orphaned'`.
- **Backend (Rust)**:
  - `services/auth.rs` / `complete_sso_callback` — unified transfer check ahead of the resolve/bind decision, covering both login and add-character call sites; seller-side fixup (re-promote or orphan); transfer audit event.
  - `db/characters.rs` — `set_main` and `promote_if_no_main` also write `account.last_known_main_*` in-tx (`promote_if_no_main` currently returns only a bool, so the caller passes `eve_character_id` + `name`, which `SsoCompletionInput` already holds); detach/rebind query; remaining-character count for the seller fixup.
  - `db/accounts.rs` — `last_known_main_*` read/write; `'orphaned'` transition; hard-delete (`DELETE FROM account`) with last-server-admin guard; blast-radius counts (characters/sessions/keys, owned maps/ACLs) for the preview.
  - `services/account.rs` / `services/admin.rs` + `handlers/api/v1/admin.rs` — admin hard-delete service + endpoint behind `AdminAccount`, returning the preview counts and performing the delete.
  - `audit/mod.rs` — new `CharacterTransferred` (and `AccountHardDeleted`) audit variants in the established `kind`-string house style.
- **FK behavior (already designed, relied on by hard-delete):** `eve_character` / `session` / `api_key` `account_id` are `ON DELETE CASCADE` (removed with the account); `map` / `acl` `owner_account_id` and `audit_log.actor_account_id` / `blocked_eve_character.blocked_by` are `ON DELETE SET NULL` (survive, un-owned/anonymized — audit history is **not** lost: actor character id + name and the JSONB `details` are snapshots).
- **Frontend (this change is NOT backend-only)**: orphaned accounts surface and are nameable via `last_known_main_*` in admin account views; owner-less maps/ACLs remain visible; the admin hard-delete UI presents the deletion preview + an explicit irreversible-confirm step. Per `CLAUDE.md`, verification runs the full frontend trio (`pnpm test`, `pnpm run check`, `pnpm run test:e2e`) from `frontend/`.
- **Tooling**: regenerate the sqlx offline cache (`cargo sqlx prepare`) and commit the `.sqlx/` diff for the schema/query changes.
- **Out of scope (follow-up):** account merge / admin character-move (the matching-hash same-human re-homing problem); self-service character release from an account.
