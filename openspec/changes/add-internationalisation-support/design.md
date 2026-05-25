## Context

The frontend has no i18n infrastructure. All user-facing strings are hardcoded in English across Svelte components. Adding i18n requires choosing a library, establishing a message catalogue convention, resolving the active locale per request, and persisting the user's locale preference.

The library is **Paraglide** (`@inlang/paraglide-js`). Paraglide is *compile-time*: each message becomes a tree-shakeable function (`m.greeting()`), and the active locale is resolved per request from a server-side context rather than held in a reactive store. This matters for SSR — the server renders the correct language on the first pass, so there is no wrong-language flash on hydration the way a runtime store-based library (svelte-i18n, i18next) would risk.

Locale **persistence** is not built from scratch here. The `account-preferences` substrate (shipped by `accessibility-preferences`) provides a `preferences` JSONB column on `account`, `GET`/`PATCH /api/v1/me/preferences` behind `AuthenticatedAccount`, and a localStorage-first-with-backend-sync frontend store with login reconciliation. Locale is stored as `preferences.locale` on that substrate — no new column, endpoint, or store.

The subtlety is that Paraglide resolves the *active* locale server-side (per request), while `preferences.locale` is the user's *saved* choice. The two are bridged by a cookie: Paraglide reads a locale cookie during SSR, and the preferences store writes that cookie whenever `locale` changes. This is the one real integration point of the change.

## Goals / Non-Goals

**Goals:**
- Adopt Paraglide as the i18n library, integrated with SvelteKit + Svelte 5
- Establish a message catalogue structure and key naming convention
- Replace **all** hardcoded user-facing strings with message keys
- Resolve the active locale per request via cookie → Accept-Language → `baseLocale` (`en`)
- Persist the locale preference as `preferences.locale` on the `account-preferences` substrate (no new column or endpoint), bridged to Paraglide's locale cookie

**Non-Goals:**
- Providing translations beyond English at this stage (infrastructure only — English is the only locale shipped)
- Locale in the URL path (no `/en/`, `/de/` prefixes) — see the resolution-strategy decision
- Right-to-left layout support

## Decisions

### Decision: Paraglide as the i18n library

Use Paraglide (`@inlang/paraglide-js`) with its SvelteKit integration. It compiles messages to tree-shakeable functions and resolves the active locale per request, which gives correct-language SSR with no hydration flash and strong type-safety on message keys. Alternatives considered (svelte-i18n, typesafe-i18n, i18next) are runtime/store-based and carry the SSR-mismatch / flash risk that our server-rendered, preference-driven setup specifically wants to avoid.

### Decision: Locale resolution strategy — cookie → Accept-Language → baseLocale (no URL prefix)

Configure Paraglide with `strategy: ['cookie', 'preferredLanguage', 'baseLocale']`:

- **cookie** — the user's chosen locale, written by the preferences store (see the bridge decision). Server-readable, so SSR renders the right language first paint.
- **preferredLanguage** — the browser `Accept-Language` header, for an anonymous first visit with no cookie.
- **baseLocale** — `en`, the final fallback.

Deliberately **no `url` strategy** (no `/en/`, `/de/` path prefixes). Rationale: E-R Bridge is an authenticated tool, not a public content site — the SEO and shareable-localized-link benefits of URL-prefixed locales do not apply, while the cost (a `[locale]` segment on every route, `localizeHref()` on every internal link, and reconciling a URL locale against the stored preference) is real and immediate when only English ships. This burns no bridges: `'url'` can be prepended to the strategy chain later if the app ever goes public.

### Decision: Locale persists as `preferences.locale` on the account-preferences substrate

Locale is stored as the `locale` key in the `preferences` JSONB bag and read/written through the existing `GET`/`PATCH /api/v1/me/preferences`. The preferences **service layer** is extended to recognise and validate `locale` (it must be one of the supported locales). This reuses the substrate's localStorage-first-with-backend-sync model and login reconciliation for free — no new column, endpoint, or store. Locale and the accessibility preferences are the same problem shape (a per-account preference that also works anonymously), so a second parallel system would be needless duplication.

### Decision: Bridge `preferences.locale` to Paraglide's locale cookie

Paraglide resolves the active locale from a cookie during SSR; `preferences.locale` is the saved choice. These must never disagree. The preferences store therefore writes Paraglide's locale cookie whenever `locale` changes — on Apply and during login reconciliation — so the server-rendered language always matches the stored preference. This is the single integration point between the i18n library and the preference substrate.

### Decision: Browser detection only sets the *default*, it does not write a preference

On first visit with no cookie and no stored `preferences.locale`, the active locale comes from `Accept-Language` (the `preferredLanguage` strategy), falling back to `en`. This is a runtime default, not a stored value — mirroring the `auto` posture of the accessibility preferences. A value is stored (and the cookie written) only when the user explicitly chooses a locale.

## Risks / Trade-offs

- **Ordering dependency on `accessibility-preferences`.** This change assumes the substrate exists. (`accessibility-preferences` is now archived, so the dependency is satisfied — but the dependency remains real for understanding the design.)
- **Cookie ⇄ `preferences.locale` consistency.** Two locale signals (Paraglide's cookie and the stored preference) must stay in sync; the store owns writing the cookie on every locale change. A drift would render SSR in one language and show the preference as another. Mitigation: a single code path (the store) sets both.
- **Locale validation lives in the shared preferences service.** Adding a supported locale later means updating the service's allowed-value set for the `locale` key (an app-layer change, no migration) *and* Paraglide's compiled locale list. Acceptable; both are app-layer.
- **The string-extraction effort is the bulk of the work.** Replacing every hardcoded literal across the frontend is large and easy to underestimate; it is the dominant task, not the library setup.
