## Context

The `eve-wormhole-mapper-foundation` change shipped a `preferences` placeholder in the user-menu dropdown (`frontend/src/lib/components/UserMenu.svelte`, a greyed-out `aria-disabled` `<span>`) with a TODO breadcrumb pointing at this change. This change gives that placeholder a destination: a `/preferences` page where users control accessibility settings.

The driving principle, set during exploration, is **accessible from day one** — we will not exclude a large segment of EVE players. That has two concrete consequences that shape the whole design:

1. **Accessibility must work for anonymous visitors**, including on the `/login` page itself, before any account exists. So preferences cannot live *only* behind `AuthenticatedAccount`.
2. **Preferences must survive across devices** for authenticated users. So they cannot live *only* in `localStorage`.

The resolution is a two-layer model: `localStorage` is the always-on edge store (instant, anonymous, no flash-of-unstyled-content), and the backend is a durability/sync layer for authenticated users.

### Verified ground truth (the stub's assumptions were partly wrong)

The proposal stub was written *as if* a `--font-base` custom property had already landed. **It had not** — there is no `--font-base` anywhere in `frontend/`. What actually exists in `frontend/src/app.css`:

```
html  → font-size: 100%      (browser default, ≈16px)
body  → font-size: 0.875rem  (≈14px, derived from html)
*     → all typography in rem (per project-infrastructure Typography requirement)
```

This is *better* than the stub imagined: because every typography rule is already `rem`-relative to `html`, the single knob for text-size is **`html { font-size }`**, not a separate custom property. Scaling `html`'s font-size cascades through every `rem` in the app automatically. We design around `html { font-size }`, not `--font-base`.

### Scope decisions (from exploration)

- **Full accessibility menu**, not a single toggle — text-size, reduce-motion, high-contrast, larger interactive targets, and a dyslexia-friendly typeface. Rationale: accessibility is a day-one value.
- **Backend-backed persistence** (not localStorage-only), via a generic preference substrate.
- The undocumented `settings` user-menu item (sibling of `preferences` in `UserMenu.svelte`) is **out of scope** and stays `aria-disabled`. This change only claims `preferences`.

## Goals / Non-Goals

### Goals

- A `/preferences` route reachable from the user-menu, working for both anonymous and authenticated users.
- A generic, reusable **preference substrate**: a JSONB bag on `account`, a CRUD endpoint, and a frontend store that is localStorage-first with backend sync.
- Five accessibility preferences, each defaulting to the relevant OS media query where one exists.
- No flash-of-unstyled-content: preferences apply before first paint.
- No lock-out: layout-altering preferences auto-revert if not confirmed, so a setting that breaks the page recovers itself with zero user effort.

### Non-Goals

- The `settings` menu item — left disabled.
- A general user-profile / account-settings page — this is scoped to accessibility.
- `prefers-color-scheme` support — the app is dark-only by design; documented as N/A.
- Implementing i18n. This change builds the substrate that i18n's locale preference will reuse, but does not add locale itself (see Cross-references).

## Decisions

### Decision: Two-layer persistence — localStorage source-of-truth at the edge, backend for cross-device sync

`localStorage` holds the canonical preference set in the browser. It is read by an inline script in `app.html` before paint and applied to `<html>`. For authenticated users, changes also `PATCH` to the backend, and the backend value is pulled on login.

This is *not* the proposal's "Stage A then Stage B" — it is both layers at once, because the day-one-accessibility goal forbids gating settings behind login.

```
   ANONYMOUS                          AUTHENTICATED
   ┌────────────────┐                 ┌────────────────┐
   │  localStorage   │ ── on first ──▶ │  localStorage   │
   │  (source of     │    login push   │      ⇅ sync     │
   │   truth)        │                 │  account.       │
   │  inline script  │   server wins   │  preferences    │
   │  → no FOUC      │ ◀── if server ──│  JSONB          │
   └────────────────┘     has prefs   └────────────────┘
```

### Decision: Conflict resolution — push-local-on-first-login, otherwise server wins

On authenticated load:
- If the server's `preferences` is empty (`{}` — a brand-new account that has never set anything) **and** localStorage has values, **push localStorage up** to the server. This preserves the setup of someone who configured accessibility while anonymous and then signed up.
- Otherwise (server already has preferences), **server wins**: pull the server value and overwrite localStorage.

This avoids needing per-key timestamps (last-write-wins was considered and rejected as over-complex for a settings page).

### Decision: Storage shape — `preferences JSONB` column on `account`, not a dedicated table

The preference set is explicitly designed to grow (five keys now, more enumerated). A JSONB bag means adding a sixth preference later is a frontend + DTO change with **zero migration**. A dedicated `account_preferences` table only earns its keep if we needed to query *across* accounts by preference value, which an accessibility page never does.

Validation lives in the application (service) layer: unknown keys are rejected, known keys are checked against their allowed enum values. The column is `NOT NULL DEFAULT '{}'::jsonb`.

```
Migration:  ALTER TABLE account ADD COLUMN preferences JSONB NOT NULL DEFAULT '{}'::jsonb;
```

### Decision: A generic preference substrate, reused by i18n

The in-progress `add-internationalisation-support` change needs the same machinery: a server-persisted, per-account preference that also works anonymously via the browser, with a browser-default-then-sync model and a before-paint application (`<html lang>` for locale, exactly parallel to `html { font-size }` for text-size).

Therefore this change builds the substrate **generically**:
- The JSONB column is a general preference bag, not accessibility-specific.
- `GET`/`PATCH /api/v1/me/preferences` operate on the whole bag (PATCH is a partial merge).
- The frontend store is a generic preferences store; accessibility keys are one consumer.

`locale` is intended to become `preferences.locale` on this same substrate. This change does not add it, but does not preclude it. See Cross-references.

### Decision: Each preference is tri-state, defaulting to the OS media query

Where an OS-level media query exists, the preference default is **Auto (follow OS)**, and "Auto" is implemented with pure CSS `@media` — **zero JavaScript and zero stored value**. A stored preference exists only to *override* the OS. This is the most accessible-by-default posture: an anonymous user with `prefers-reduced-motion` at the OS level gets reduced motion with nothing stored at all.

| Preference     | Values                          | Default | "Auto" mechanism                          | Applied via                          |
|----------------|---------------------------------|---------|-------------------------------------------|--------------------------------------|
| `text_size`    | `auto` / `small` / `regular` / `large` | `auto`  | `html { font-size: 100% }` (browser pref) | `html { font-size }` override        |
| `reduce_motion`| `auto` / `on` / `off`           | `auto`  | `@media (prefers-reduced-motion: reduce)` | `data-reduce-motion` attr on `<html>`|
| `high_contrast`| `auto` / `on` / `off`           | `auto`  | `@media (prefers-contrast: more)`         | `data-high-contrast` attr on `<html>`|
| `large_targets`| `off` / `on`                    | `off`   | (no OS query)                             | `data-large-targets` attr on `<html>`|
| `dyslexia_font`| `off` / `on`                    | `off`   | (no OS query)                             | `data-dyslexia-font` attr on `<html>`|

`text_size` is applied by setting `html { font-size }` (`small`≈87.5%, `regular`=100%, `large`≈125%; exact steps finalised during implementation). The other four are applied as `data-*` attributes on `<html>` that CSS keys off. `auto` means **no attribute / no override**, so the CSS `@media` default takes over.

### Decision: No FOUC — an inline script in `app.html` applies preferences before paint

`app.html` gains a small inline `<script>` in `<head>` that reads `localStorage`, and for any non-`auto` value, sets `html { font-size }` and the relevant `data-*` attributes on `document.documentElement` synchronously before the body renders. The SvelteKit app then hydrates the store from the same source (and, for authenticated users, reconciles with the server).

This is the standard dark-mode-flash fix applied to accessibility preferences. It is the reason `localStorage` (synchronously readable) is the edge store rather than the backend (async).

### Decision: Layout-altering preferences use an auto-reverting confirmation ("safe mode")

A preference that changes visual layout or rendering can be **self-defeating**: setting `text_size: large` may push the confirm control off-screen; a mis-mapped `high_contrast` token could hide text; `large_targets: on` could overlap controls. If the very control needed to undo the change is broken *by* the change, the user is locked out.

Borrowing the OS display-settings pattern (Windows' "Keep these display settings?" that reverts after 15s), every **layout/render-altering** preference change is applied as a **live preview** and then either committed or automatically rolled back:

```
  User changes text_size → large
  ┌─────────────────────────────────────────────────┐
  │  ⚠ Preview applied.                              │
  │     Keeping these settings in  [ 9 ]s …          │
  │     [ Keep ]            [ Revert now ]           │
  └─────────────────────────────────────────────────┘
        │                          │
   Keep / Revert now        countdown → 0, no click
        │                          │
        ▼                          ▼
   commit OR restore        AUTO-REVERT to previous value
```

**Which preferences get the countdown:** the layout/render-altering ones only — `text_size`, `high_contrast`, `large_targets`, `dyslexia_font`. `reduce_motion` is **excluded**: it is purely subtractive (removing motion cannot lock a user out), so a countdown there is pointless friction.

**Countdown duration:** default **10 seconds**, defined as a single named constant (e.g. `PREFERENCE_REVERT_SECONDS`) so it can be tuned in one place — not a magic number scattered through the UI.

**Commit timing is the crux of the safety guarantee:** the change applies live to `<html>` for preview, but is written to `localStorage` and `PATCH`ed to the backend **only on "Keep"**. During the countdown nothing is persisted, so a reload (or crash) mid-countdown shows the *old, safe* value. A lock-out setting can therefore never persist across reloads — doing nothing always returns the user to safety with zero effort.

**The confirmation control must resist the preview it shows.** The Keep / Revert dialog should be styled so the previewed change cannot break it — e.g. fixed `px` sizing rather than `rem` (so `text_size: large` doesn't enlarge the dialog itself off-screen), and a guaranteed-high-contrast palette (so `high_contrast` previews can't hide the buttons). Otherwise the escape hatch is subject to the same trap.

### Decision: Reduce-motion requires auditing every animation/transition

Honouring `reduce_motion` means every animation and transition in the codebase must be gated. The known offenders from the proposal: the pulsing `connected` dot animation and the character-grid hover transitions. The implementation includes an audit pass that wraps motion in a mechanism that respects both `@media (prefers-reduced-motion)` (the `auto` default) and the `data-reduce-motion="on"` override.

## API shape

```
GET   /api/v1/me/preferences        → 200 { data: { text_size, reduce_motion, ... } }
                                       (AuthenticatedAccount; 401 if unauthenticated)
PATCH /api/v1/me/preferences         → 200 { data: { ...merged preferences } }
   body: a partial set of keys to merge; unknown keys → 400; bad enum value → 400
                                       (AuthenticatedAccount)
```

Both endpoints live under the existing `me` handler module (`backend/src/handlers/api/v1/`), mirroring `get_me`. The response uses the standard `ApiResponse` / `ErrorEnvelope` envelope. Backend module layout follows the `rust-rest-api` skill: `handlers/api/v1/me.rs` (or a new `preferences.rs`) → `services/preferences.rs` → `db/preferences.rs` → `dto/preferences.rs`.

## Risks / Trade-offs

- **JSONB validation is app-level, not DB-level.** A bad write bypassing the service (e.g. direct SQL) could store a garbage value. Mitigation: all writes go through the service's validation; the inline `app.html` script tolerates unknown values by ignoring them.
- **Two changes touch `<html>` before paint.** Both this change (`font-size`, `data-*`) and i18n (`lang`) want the inline-script slot in `app.html`. Building the substrate generically (and noting it for i18n) is the mitigation; the inline script is written to be extendable.
- **Reduce-motion audit can miss an animation.** New animations added later won't automatically respect the preference. Mitigation: document the motion-gating mechanism so new code uses it; the project-infrastructure spec amendment is the place to record the rule.
- **`large_targets` and `dyslexia_font` have no OS default**, so they're plain on/off. Lower priority; included for completeness of the day-one menu but the simplest to implement.

## Migration

- One forward migration adds `account.preferences JSONB NOT NULL DEFAULT '{}'::jsonb`. Existing accounts get `{}`, meaning "all auto/defaults" — no behavioural change for current users until they set something.
- No data backfill needed.

## Open questions (resolve during implementation)

- Exact `text_size` percentages for `small` / `large`.
- Which dyslexia-friendly typeface to bundle (and licensing).
- Whether `/preferences` should be publicly reachable (like `/about`) or require auth. Leaning **public** — anonymous users must be able to set accessibility on the login page — with the page rendering the same controls either way, only differing in whether changes sync to a server.
