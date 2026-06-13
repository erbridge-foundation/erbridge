## Context

The accessibility/locale preference system is already fully built: a localStorage-first store (`src/lib/preferences/store.svelte.ts`), a DOM applier (`apply.ts`), a no-FOUC inline bootstrap in `app.html`, a public `/preferences` page with General (locale) and Accessibility tabs, and backend sync for authenticated users. Crucially, the bootstrap and applier run on **every** route, so preferences already take effect on the login page — there is nothing to build on the rendering side.

The only gap is discoverability *before* sign-in: `/preferences` is reachable from the user menu, which doesn't exist for an unauthenticated visitor, and the login card (`src/routes/login/+page.svelte`) offers only the SSO button. A low-vision or first-time user cannot adjust the login screen they're looking at.

This change surfaces the existing system on the login card. It is purely additive composition — no new store, endpoint, preference key, persistence path, or auth-flow change.

## Goals / Non-Goals

**Goals:**
- One-tap relief: a single "Maximize accessibility" toggle that applies a known-good high-accessibility preset and is reversible in place.
- Read-the-login-screen-now language selection (en/de/fr) using the already-translated login strings.
- Honest framing: "applies to this screen," with guidance that settings live under User Menu › Preferences after sign-in.
- Reuse `preferences.commit` and the existing locale→Paraglide bridge verbatim.

**Non-Goals:**
- No change to `reconcile()` precedence. Existing users' account preferences override a login-page tweak on sign-in; this is intended.
- No server-side capture of preferences via the OAuth `state` / in-flight record. The client-side localStorage→account promotion already covers the first-timer case; the in-flight path was considered and rejected as gold-plating that would expand the recently-hardened auth surface.
- No new preference keys, controls beyond these two, store changes, or backend changes.
- No clickable link to `/preferences` from the login page (the destination requires a session).
- No per-control accessibility editing on the login card — that richness stays in `/preferences`.

## Decisions

### Decision: Drive the controls through `preferences.commit`, not a new mechanism
`commit(patch)` already does exactly what's needed: updates in-memory state, applies to `<html>`, writes localStorage, and fires the backend sync (which silently no-ops without a session). On the login page there is no session, so `commit` degrades to "apply + localStorage" automatically.

- *Alternative considered:* `preview()` (apply without persisting). Rejected — the login-page intent is to *keep* the setting through the SSO round-trip so `reconcile()` can promote it on first login; preview would be dropped on navigation.

### Decision: "Maximize accessibility" is a toggle whose state is derived from the store
The control is "on" iff the current preference set equals the preset exactly (`text_size: large`, `high_contrast/reduce_motion/large_targets/dyslexia_font: on`). Activating commits the preset; deactivating commits those five keys back to their **defaults** (`text_size: auto`, the rest `auto`/`off`).

- *Rationale:* A one-way "max" with no visible off is itself an accessibility trap. Deriving on/off from the store keeps the UI honest if the user previously set some of these in `/preferences`.
- *Trade-off:* "Off" reverts to defaults, not to whatever the user had before tapping (we don't track pre-tap state). Simple and predictable for a login-screen affordance; full granularity remains in `/preferences`.
- `locale` is deliberately excluded from the preset so the language picker and the accessibility toggle are independent.

### Decision: The preset lives as a named constant
Define `MAX_PREFERENCES` (the five-key preset) as a constant so the apply-on and the on-state-derivation reference one source of truth. Co-locate it where the schema/defaults live (`src/lib/preferences/`) or local to the login route; either keeps it a single definition the test can assert against.

### Decision: Language picker reuses the existing locale bridge
Selecting a locale calls `commit({ locale })`. The store's `bridgeLocale(..., reload: true)` writes the Paraglide cookie and reloads so the new language renders. On the login route a reload is harmless (no form in progress). The login strings are already translated (`login_title`, `login_subtitle`, `login_disclaimer_*`, `login_sso_aria`).

### Decision: Preferences reference is guidance text, not a link
The destination (`/preferences`) is reachable from the user menu, which only exists post-login. A login-page link would dangle for the unauthenticated visitor. Instead, render a short breadcrumb-style hint ("User Menu › Preferences") as plain text, teaching the location for later.

### Decision: New strings are full Paraglide messages in all three locales
Consistent with the rest of the login card and the project's locale-sync rule (en/de/fr kept in step). Keys: preset label, preset on-state/disclosure text, language-selector label, and the post-login guidance.

## Risks / Trade-offs

- **[Card clutter]** The login card is deliberately minimal; adding two controls + guidance risks turning it into a control panel → Keep the controls compact and below the SSO button/disclaimer so the primary action stays dominant; reuse existing card spacing tokens.
- **[Surprising override for returning users]** A returning user who tweaks language/accessibility on login sees it snap back to their account prefs after sign-in → Accepted and mitigated by copy: controls are framed as "applies to this screen," and the guidance points to Preferences for durable changes. No behaviour change to `reconcile()`.
- **[Reduce-motion in a "max" bundle]** Forcing `reduce_motion: on` and `dyslexia_font: on` on anyone who taps "maximize" is blunt → Accepted by explicit product decision: "maximize" means maximize; each is one toggle-off away in `/preferences`.
- **[Preset drift]** If new accessibility keys are added later, the preset and its tests must be updated → The single `MAX_PREFERENCES` constant + a Vitest assertion against it localizes that maintenance.

## Migration Plan

No data migration. Frontend-only, additive. Deploy is a standard frontend build; rollback is reverting the login route + message changes. No schema, backend, or persisted-format change, so no forward/backward compatibility concerns.

## Open Questions

None outstanding. Preset contents, toggle semantics (off → defaults), and the guidance-not-link decision are settled.
