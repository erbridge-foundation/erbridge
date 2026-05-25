## Context

The frontend has no i18n infrastructure. All user-facing strings are hardcoded in English across Svelte components. Adding i18n requires choosing a library, establishing a message catalogue convention, wiring locale detection, and persisting the user's locale preference.

Locale persistence is **not** built from scratch here. The `accessibility-preferences` change introduces a generic preference substrate — a `preferences` JSONB column on `account`, `GET`/`PATCH /api/v1/me/preferences` behind `AuthenticatedAccount`, a localStorage-first-with-backend-sync frontend store, and an apply-before-paint bootstrap in `app.html`. Locale is conceptually just another preference with the same shape (anonymous browser default, server override for authenticated users, applied to `<html>` before paint — `lang` here, exactly as `text_size` sets `html { font-size }` there). This change therefore stores locale as `preferences.locale` on that substrate rather than adding its own column, endpoint, and store.

## Goals / Non-Goals

**Goals:**
- Introduce a lightweight i18n library compatible with SvelteKit and Svelte 5 runes
- Establish a message catalogue structure and key naming convention
- Replace hardcoded user-facing strings with translation calls
- Detect and apply browser locale on first visit
- Persist the locale preference as `preferences.locale` on the `account-preferences` substrate (no new column or endpoint)

**Non-Goals:**
- Providing translations beyond English at this stage (infrastructure only)
- Right-to-left layout support
- Server-side rendering of translated content beyond what SvelteKit provides by default

## Decisions

### Decision: Locale persists as `preferences.locale` on the account-preferences substrate

Rather than a dedicated locale column + endpoint, locale is stored as the `locale` key in the `preferences` JSONB bag and read/written through the existing `GET`/`PATCH /api/v1/me/preferences`. The preferences **service layer** is extended to recognise and validate `locale` (it must be one of the supported locales). This reuses the substrate's localStorage-first-with-backend-sync model and login reconciliation (server wins, else push-local-on-first-login) for free.

Rationale: locale and the accessibility preferences are the same problem shape — a per-account user preference that must also work anonymously via the browser and apply before paint. Two parallel preference systems on `account` would be needless duplication.

### Decision: Locale applies to `<html lang>` before paint via the shared bootstrap

The `app.html` inline bootstrap that `accessibility-preferences` adds (reading `localStorage`, applying values to `document.documentElement` before the body renders) is extended to also set `<html lang>` from `preferences.locale`. This keeps locale on the same no-FOUC path as the other preferences, and avoids a flash of the wrong language / a layout shift on hydration.

### Decision: Browser detection only sets the *default*, it does not write a preference

On first visit with no stored `preferences.locale`, the active locale is derived from the browser (`navigator.language`), falling back to `en`. This is a runtime default, not a stored value — mirroring the `auto` posture of the accessibility preferences. A value is stored only when the user explicitly chooses a locale.

## Risks / Trade-offs

- **Ordering dependency on `accessibility-preferences`.** This change assumes the substrate exists. If i18n is implemented first, it must build the substrate itself (defeating the consolidation). Mitigation: apply `accessibility-preferences` first; the dependency is stated in the proposal.
- **Locale validation lives in the shared preferences service.** Adding a supported locale later means updating the service's allowed-value set for the `locale` key (an app-layer change, no migration). Acceptable given the substrate is explicitly app-validated.
- **`<html lang>` set by an inline script** can momentarily disagree with SvelteKit's SSR-rendered `lang` attribute. Mitigation: the bootstrap runs before paint and SvelteKit re-asserts the same value on hydration from the same source.
