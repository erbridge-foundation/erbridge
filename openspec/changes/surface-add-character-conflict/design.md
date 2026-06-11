# Design — surface-add-character-conflict

## Context

`accounts::resolve_or_create` short-circuits in add-character mode (returns the session's account without looking at the character), and `characters::upsert_tokens`' `ON CONFLICT` CASE keeps an existing different `account_id` while still overwriting that row's tokens, names, and `owner_hash`. `complete_sso_callback` then emits `CharacterAdded { account_id: session_account }` — wrong on two counts: the character was not added, and the tokens of a row belonging to a *different* account were overwritten by this session's flow (harmless in practice — they're the character's own fresh tokens — but a write to another account's row that the session has no claim to).

The blocked-character flow already models the right shape: a non-error, non-happy-path outcome (`SsoOutcome::Blocked`) that the handler maps to an informational redirect.

## Goals / Non-Goals

**Goals:**
- The bound-elsewhere case is detected before any write to the other account's row.
- The user sees an explanation; the audit log records what actually happened.

**Non-Goals:**
- Character *transfer* between accounts (a future feature; this change only refuses cleanly).
- Changing the ordinary login flow for a bound character (logging in as that character correctly resolves its own account — untouched).
- Changing `upsert_tokens`' keep-existing-binding CASE (it remains the backstop; the service-level check makes it unreachable in this flow).

## Decisions

**Detect in `complete_sso_callback`, inside the transaction.** The add-character branch already calls `find_account_id_for_eve_character` (for the orphan-vs-fresh audit distinction). Extend that lookup's use: if the character row exists with `Some(other_account)` where `other_account != session_account`, roll back and return a new `SsoOutcome::BoundElsewhere`. No token write occurs because the check precedes `upsert_tokens`. Alternative considered: pre-check in the handler before the service call — rejected; the decision belongs with the transaction so it cannot race a concurrent claim.

**Audit the rejected attempt.** Mirroring `blocked_login_rejected` (the established "rejected attempt" precedent in the catalogue), emit `character_add_rejected_bound_elsewhere` with actor = session account, target = the character. Recorded in its own short transaction after rollback of the main one, exactly as the blocked flow does.

**Redirect, not error page.** `SsoOutcome::BoundElsewhere` → 303 redirect to `{return_to or /characters}?add_conflict=bound_elsewhere`. The characters page reads the query flag and renders a dismissible notice. Alternative considered: a dedicated `/add-conflict` interstitial like `/blocked` — heavier than needed; the user's natural destination is the characters page they came from, and `/blocked` exists because the blocked user has no authenticated destination at all (no session), which is not the case here.

**Session is preserved.** Unlike `/blocked`, the user keeps their session — the conflict concerns the character, not the caller.

## Risks / Trade-offs

- [Query-param UX] The flag survives reloads until the URL is cleaned. → The notice is dismissible and the page replaces the URL (`history.replaceState` via SvelteKit's `replaceState`) after rendering it once.
- [Token freshness for the other account] Today's silent path at least refreshed the bound character's tokens; refusing means it no longer does. → Correct behaviour: the other account refreshes its own tokens by its own logins/sweep.
- [Race with a concurrent unlink] The other account removes the character between SSO start and callback → lookup finds an orphan or nothing, and the normal claim/add path proceeds. Benign in both directions because the check is transactional.

## Migration Plan

Single deploy, backend + frontend together (the redirect target needs the notice). No data migration. Rollback is a revert.
