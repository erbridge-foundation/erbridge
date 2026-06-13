## Context

The backend's `block_character` service rejects an admin blocking any character on their own account with `409 cannot_block_self`, keyed on the **owning account** (so it catches alts, not just the main). The admin block picker at `/admin/blocks` does not reflect this: the admin's own characters render as ordinary selectable rows, so the only feedback is a failed action after select → corp-lookup → confirm → submit.

The picker already has a "non-actionable result" idiom: results with `already_blocked` render a small badge instead of the Select form (`+page.svelte` `resultList` snippet). The current account is already available to the page via the root layout load (`data.me`, an `MeResponse` carrying `account.id` and `characters[]`). So the affordance is a small, self-contained frontend change with no new data plumbing.

## Goals / Non-Goals

**Goals:**
- Mark the logged-in admin's own characters as non-selectable in the picker, with a short explanation on hover.
- Match the backend's account-level semantics as closely as the available client data allows (catch alts, not just the main).
- Reuse the existing badge idiom so the change is visually and structurally consistent.

**Non-Goals:**
- No change to the backend guard or any request-path behaviour — the `409` remains the enforcement boundary.
- No new "who am I" endpoint or extra load — `data.me` already carries everything needed.
- No change to the already-blocked or unblock flows.

## Decisions

### Decision: A non-actionable "You" badge, not a disabled button
The self row renders a `<span class="self-badge">` ("You") in place of the Select form, exactly mirroring the `already_blocked` branch. The reason text lives in a `title` on the badge.

- **Why:** Consistency with the existing badge pattern; a real `disabled` button has unreliable native-tooltip behaviour on hover across browsers and adds an accessibility wrinkle. A badge is the idiom already in the file.
- **Alternative considered:** a greyed-out `disabled` Select button with a tooltip (the original ask). Rejected for the disabled-hover quirk and inconsistency with the blocked badge.
- **Colour:** `.self-badge` uses a neutral slate tone, not the red of `.blocked-badge` — "You" is a non-actionable state, not a warning about a bad actor.

### Decision: Render order — blocked, then self, then selectable
The `resultList` branch order is `already_blocked` → `isSelf` → Select form. If one of the admin's own characters were somehow already blocked, the stronger statement of fact (already blocked) wins.

### Decision: Self-match — account for local, character for ESI
`isSelf(result)` against `data.me`:
- Local results (`CharacterSearchResultDto`, has `account_id`): `result.account_id === me.account.id`. This mirrors the backend's account-level guard exactly and catches every alt.
- ESI results (`EsiCharacterSearchResultDto`, no `account_id`): `me.characters.some(c => c.eve_character_id === result.eve_character_id)`.

`'account_id' in result` is the TypeScript discriminant between the two DTOs and narrows cleanly. An admin's own character is by definition in their account and therefore present in `me.characters`, so the ESI char-level fallback fully covers the admin's known characters.

## Risks / Trade-offs

- **[Cosmetic-only, not a security control]** → The badge is defence-in-depth; correctness still rests on the backend `409`. The spec is explicit that enforcement is unchanged, so a bypassed/disabled UI changes nothing about safety.
- **[ESI match can't see an account_id]** → An ESI result is matched by `eve_character_id` against `me.characters`. This catches every character the admin actually owns (all are in `me.characters`); there is no realistic case where an admin's own character is returned by ESI search yet absent from their own `me.characters`.
- **[`data.me` could be null]** → The page is admin-gated, so `me` is present in practice; `isSelf` guards with optional chaining and falls back to "not self" (selectable) if `me` is somehow absent, which is the safe default given the backend still enforces.
