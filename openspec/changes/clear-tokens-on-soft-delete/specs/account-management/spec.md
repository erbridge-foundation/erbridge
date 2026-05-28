## ADDED Requirements

### Requirement: Soft-delete token-clearing policy is explicitly defined

The `account-management` capability SHALL state, explicitly and unambiguously, what happens to `eve_character.encrypted_access_token` and `eve_character.encrypted_refresh_token` (and any related columns: `access_token_expires_at`, `scopes`) when an account is soft-deleted via `DELETE /api/v1/account`, and what happens to them when the soft-deleted account is reactivated via SSO re-login.

**STUB.** This requirement is a placeholder for an unresolved spec question surfaced during the `add-account-page-and-api-keys` change. The normative text below describes the *meta*-requirement (the spec must answer the question); the *specific* answer SHALL be filled in via an explore + design session before this change is implemented. Do not implement this change as-is.

The current spec text ("Character rows SHALL NOT be modified") is ambiguous about whether *columns* of those rows are zeroed even though the row itself is kept, and the current implementation interprets that as "leave tokens intact." This stub records the question; the answer is to be decided.

Candidate positions to evaluate in design.md:

1. **Status quo** — keep both tokens intact on soft-delete; reactivation restores a fully-usable account with zero re-auth friction.
2. **Clear both tokens** — zero `encrypted_access_token` and `encrypted_refresh_token` on soft-delete; reactivation requires re-doing SSO for every linked character.
3. **Clear access token only** — zero `encrypted_access_token` but keep `encrypted_refresh_token`; matches the spec's existing rule that `token_status` derives from refresh-token presence, and matches what an attacker would care about (refresh tokens are the durable credential).
4. **Distinguish user-initiated from admin-initiated soft-deletes** — the question may have different answers in the two cases; this is purely a placeholder, the existing endpoint is user-initiated only.

#### Scenario: Stub placeholder — policy is stated, not implied
- **WHEN** a reader of `account-management` looks up the soft-delete behaviour for encrypted token columns
- **THEN** the spec provides an unambiguous answer (one of the candidate positions above, or another reached during explore), with at least one scenario each for the soft-delete write and the reactivation read

#### Scenario: Stub placeholder — reactivation behaviour matches the chosen policy
- **WHEN** a soft-deleted account reactivates via SSO re-login under the chosen policy
- **THEN** the resulting `token_status` for each character matches what the chosen policy implies (e.g. all `"expired"` under position 2 and 3; mixed under position 1 if other factors apply)
