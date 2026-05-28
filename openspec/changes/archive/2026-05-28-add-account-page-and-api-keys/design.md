## Context

Backend API key endpoints are live per `api-authentication`:

- `POST /api/v1/keys` — creates an account-scoped key; returns the plaintext exactly once.
- `GET /api/v1/keys` — lists the caller's keys (metadata only; never plaintext).
- `DELETE /api/v1/keys/:id` — revokes a key the caller owns.

The contract has one dominant UX constraint baked in: *the plaintext key is shown once and never again*. Any frontend that surfaces it has to assume "the user might lose this between the response arriving and the next render" and design around that.

The user-chip menu (`frontend/src/lib/components/UserMenu.svelte`) already carries a disabled span labelled "Settings" (i18n key `user_menu_settings`). The slot exists, the i18n exists, the visual treatment exists — only the `href` and the live-vs-disabled classing are missing. The visible label needs to change (Settings → Account) because the new page is account-scoped, not a generic settings hub.

`/characters` currently hosts a "Danger zone" section containing the "Delete account" action. The placement was pragmatic (it was the only authenticated page that needed it) but conceptually wrong: account lifecycle is not character management. With `/account` arriving, this move becomes correct *and* cheap.

Two precedents in the codebase fully determine the page shape:

- **`/characters`** — SSR loader + form actions + `use:enhance`; `ConfirmDialog` for destructive actions; `bind:this` on the form so the modal's `onConfirm` can call `formEl.requestSubmit()`.
- **`/preferences`** — tabs implemented as local `$state<Tab>` with conditional rendering, not as nested routes. Tab state is not URL-backed.

The `frontend-patterns` capability mandates `ConfirmDialog` for every destructive frontend action; inline confirmations, `window.confirm()`, and ad-hoc full-page confirmation routes are forbidden. The `sveltekit-node` skill is the authoritative source for frontend file layout, runes, native CSS (no Tailwind), and the design-token system.

## Goals / Non-Goals

**Goals:**
- Ship a usable `/account` page that lets a logged-in account create, list, and revoke API keys, and (via the moved action) soft-delete itself.
- Honour `api-authentication`'s one-shot plaintext invariant via a confirmation gate the user must actively pass through before the plaintext disappears.
- Match `/characters`/`/preferences` patterns line-for-line for routing, tabs, forms, and destructive-action gating — no new architectural shapes.
- Move the existing delete-account UI without changing the soft-delete semantics or the visible copy.
- Activate the user-chip menu slot already present, with corrected naming.

**Non-Goals:**
- A sessions/active-logins surface (deferred until SSE map-events work introduces session-as-connection semantics).
- Server-scoped API key management (out-of-band per `api-authentication`).
- "Last used at" telemetry on keys (would require a backend column + endpoint change).
- Renaming `/preferences` (preferences ≠ account; they remain sibling destinations).
- Any change to the soft-delete flow itself.
- A `window.confirm`-free fallback for no-JS users — the existing accepted regression for `/characters` destructive actions carries over without re-litigation.

## Decisions

### D1: New route at `/account`, single route, tabs in local state

`frontend/src/routes/account/+page.{server.ts,svelte}` — one route, two tabs (`api-keys`, `danger-zone`) rendered conditionally on a `let activeTab = $state<Tab>('api-keys')` variable. Tab state is **not** URL-backed.

*Alternatives considered:* nested routes (`/account/keys`, `/account/danger`); URL query param (`?tab=keys`). Rejected: `/preferences` already establishes "local state, no URL backing" as the project's tab idiom. Diverging here would add inconsistency for no user benefit (deep-linking to a specific tab is not a need — both tabs are short, both fit on the same page, and the destructive tab arguably *shouldn't* be deep-linkable from outside).

### D2: One-shot plaintext is held in component-local `$state`, not in `form` only

After a successful create, the form action returns `{ createdKey: CreatedKeyDto }` via the SvelteKit `form` prop. We **also** stash it into a local `$state` variable:

```
let revealedKey = $state<CreatedKeyDto | null>(null);
```

When `form?.createdKey` arrives, an `$effect` copies it into `revealedKey` and triggers `invalidateAll()` so the list refreshes. The reveal panel renders from `revealedKey`, not from `form`. Dismissing the panel (the "Done" button) sets `revealedKey = null`.

*Why not just use `form`?* `invalidateAll()` reruns the load and clears `form` to `null` on the next render. The user would see the reveal panel flash and disappear. Holding it in `$state` decouples the plaintext-survival lifetime from SvelteKit's form-result lifetime.

*Why not store it in `sessionStorage` for refresh-survival?* The spec mandates the plaintext appears once. Persisting it across reload would weaken that. The user must copy it before clicking Done; if they refresh in between, they have to revoke and re-create. This is the documented, deliberate trade.

### D3: The confirmation gate is a checkbox + disabled button, not a typed string or a timer

The reveal panel UX:

```
┌─ Key created ────────────────────────────────────┐
│  ⚠ This is the only time you'll see this key.    │
│  Copy it now — we can't show it again.           │
│                                                   │
│   erb_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx           │
│                                          [ Copy ] │
│                                                   │
│   ☐  I've saved this key somewhere safe           │
│                                                   │
│                                          [ Done ] │
└───────────────────────────────────────────────────┘
```

- `[Copy]` writes to clipboard via `navigator.clipboard.writeText` (Svelte 5; runs only in the browser branch of an `$effect`-or-event-handler, never SSR).
- `[Done]` is `disabled` while the checkbox is unchecked. Activating it sets `revealedKey = null`.
- Closing the browser tab while the panel is open is acceptable data loss — that is the *only* way the key can disappear, and that path requires the user to click outside the app entirely.

*Alternatives considered:* a "type the key to confirm you've copied it" gate (too friction-heavy and the user just round-trips clipboard → input); an auto-dismiss timer (silent loss is the failure mode we're trying to prevent); modal-with-backdrop-click-to-dismiss (a stray click would destroy the value). The checkbox+button pattern is theatre-but-deliberate: it forces a read of the warning and a deliberate two-action exit.

### D4: Revoke uses `ConfirmDialog`; revoke is one-step on click of the confirm button

Per `frontend-patterns`. The implementation mirrors `/characters` `?/remove`:

- One `<form method="POST" action="?/revoke">` per row, with `bind:this` into a map keyed by key id.
- The row's Revoke button is `type="button"`; clicking it opens the dialog. The dialog's `onConfirm` calls `formEl.requestSubmit()`.
- The form action calls `deleteKey(...)` and reruns the loader on success; on failure it returns `{ keyId, code, message }` via `fail()` and the page renders an inline error row beneath the offending row, matching the `/characters` `formError?.characterId === character.id` pattern.

### D5: Move delete-account by lift-and-shift, but rename the i18n keys

The action moves verbatim: the `?/deleteAccount` form action body, the `ConfirmDialog` block, the trigger button. What changes:

- **Host**: `frontend/src/routes/account/+page.{server.ts,svelte}` (Danger zone tab) instead of `frontend/src/routes/characters/+page.{server.ts,svelte}`.
- **i18n keys**: `characters_delete_account*` → `account_delete_account*` (values identical). Rationale: keys reflect where the copy lives; leaving them under `characters_*` after the move would mislead future readers grepping for the strings. This is internal-only churn — paraglide keys aren't part of any external contract.

The five affected keys (per `frontend/messages/{en,de}.json`):

- `characters_delete_account` → `account_delete_account`
- `characters_delete_account_title` → `account_delete_account_title`
- `characters_delete_account_body` → `account_delete_account_body`
- `characters_delete_account_confirm` → `account_delete_account_confirm`
- `characters_danger_zone` → `account_danger_zone` (the section heading also moves)

### D6: Expiry input — presets compute timestamps in the frontend

The backend takes `expires_at: RFC3339 | null`. The create form shows:

- Never (`null`)
- In 30 days, in 90 days, in 1 year (`new Date(Date.now() + N).toISOString()`)
- Custom (date-only `<input type="date">`, parsed to RFC3339 at end-of-day UTC)

The form action receives `expires_at` as already-computed RFC3339 (or empty for null). The server-side action validates and forwards to `createKey(...)`.

*Why frontend-computed?* Backend doesn't need to know about presets, and there's no advantage to round-tripping a "preset id" through the form action when the timestamp is trivial to compute. Keeps the backend contract clean.

### D7: i18n key rename for the menu item — both key AND value change

`user_menu_settings` → `user_menu_account`; en `"settings"` → `"account"`; de `"Konfiguration"` → `"Konto"`. Both files updated together. `UserMenu.svelte` uses `m.user_menu_account()`.

### D8: API client functions follow the existing `request<T>()` pattern

Added to `frontend/src/lib/api.ts` exactly mirroring `setMainCharacter` / `deleteCharacter`:

```
export function listKeys(fetch, backendUrl, cookie): Promise<KeyMetadataDto[]>
export function createKey(fetch, backendUrl, body: CreateKeyRequest, cookie): Promise<CreatedKeyDto>
export function deleteKey(fetch, backendUrl, keyId: string, cookie): Promise<void>
```

`request<T>()` already does envelope-unwrapping, `204`-handling, and `ApiError` mapping — no new infra needed. Cookie forwarded by the SSR loader / form action just like every other authenticated call.

### D9: Empty state renders the same primary CTA as the populated view

```
   No API keys yet.

   API keys let scripts and integrations talk to E-R Bridge on your
   behalf. Treat them like passwords.

                                            [ Create your first ]
```

The CTA is the same `[+ New key]` button as the populated view, just relabelled. No new state machine for empty vs. populated — both render the same create form when toggled open.

### D10: Expired keys are shown, de-emphasised, with an explicit badge

Backend doesn't auto-delete expired rows; `GET /keys` returns them. Frontend:

- Compares `expires_at` against `Date.now()` at render time.
- Expired rows render with `--slate-500`-ish foreground and an `Expired` badge in the Expires column.
- Revoke button remains active on expired rows (cleanup affordance).

Users want to know "I made a key that expired so my integration is dead" — hiding them makes diagnosis harder.

## Risks / Trade-offs

- **R1 — User dismisses the reveal panel without copying.** Mitigation: the checkbox-gated [Done] button forces a deliberate exit; copy auto-attempt would be silent and worse. Documented accepted failure mode: lost key → revoke + recreate. (D3)
- **R2 — `invalidateAll()` races with the reveal panel render.** If the loader rerun finishes before the `$effect` that copies `form.createdKey` into `revealedKey`, the panel might never appear. Mitigation: copy *first*, then `invalidateAll()`. Verified by ordering in the `$effect`. (D2)
- **R3 — Custom date picker emits local-timezone dates; backend expects UTC RFC3339.** Mitigation: at submit, compute `new Date(yyyy-mm-dd + 'T23:59:59Z').toISOString()`. End-of-day UTC matches user intent ("expires that day") and avoids ambiguity. (D6)
- **R4 — Renaming i18n keys breaks German-locale users mid-session.** Mitigation: both `en.json` and `de.json` are updated atomically with the code rename; paraglide is build-time-typed so a key/value mismatch is a compile error, not a runtime regression. (D7)
- **R5 — `/characters` regression: removing the danger zone may leave dead i18n strings.** Mitigation: the rename in D5 *is* the removal — the old keys cease to exist; new `account_*` keys are introduced. The `characters_*` i18n entries are deleted in the same diff.
- **R6 — `navigator.clipboard.writeText` rejects in non-secure contexts.** Mitigation: dev runs over Traefik with TLS (per project infra); production is HTTPS. Add a defensive `try/catch` that surfaces an inline "couldn't copy — select the key manually" message rather than failing the gate.
- **R7 — Tabs in local state lose their position on `invalidateAll()`-driven reruns.** Mitigation: SvelteKit reruns the *loader*; component `$state` survives. Verified by `/preferences` behaviour where tab state persists across staged preference applies.
- **R8 — The reveal panel is the only place rendering plaintext.** It must not be log-able. Mitigation: keep it in `$state` only; no `console.log`; the `data-testid` (if any) wraps the panel, not the value.

## Open Questions

- None. The two questions raised in the proposal are resolved by D1 (single route, local tab state) and D5 (rename keys to `account_delete_account*`).
