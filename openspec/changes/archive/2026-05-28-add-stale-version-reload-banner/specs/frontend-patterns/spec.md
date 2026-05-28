## ADDED Requirements

### Requirement: A stale-version reload banner prompts users running an outdated UI build

The frontend SHALL detect when the deployed UI version differs from the version the running tab was built with, and SHALL surface a non-destructive prompt offering the user a reload. Detection SHALL use SvelteKit's built-in version mechanism — `kit.version.name` set to the git-derived `APP_VERSION` (the same value the frontend inlines as `PUBLIC_UI_VERSION`; see the `release-versioning` capability) and `updated.current` from `$app/state` — and SHALL NOT hand-roll a polling loop or a custom version endpoint.

`kit.version.pollInterval` SHALL be set to a positive value so staleness is detected in the background (not only on navigation). When `APP_VERSION` is unset at build time (a plain local build), `version.name` SHALL fall back to a fixed, stable string so local development does not spuriously prompt on every rebuild.

The prompt SHALL be a single shared component (canonical path: `frontend/src/lib/components/UpdateBanner.svelte`, per the `sveltekit-node` skill's component-layout rule), mounted once in the root layout (`frontend/src/routes/+layout.svelte`) so it spans all routes. The component SHALL render only when `updated.current` is `true`.

The reload SHALL be user-initiated (`location.reload()` on activating the banner's reload control). The app SHALL NOT reload silently or automatically on navigation or on a timer, so that in-progress, unsaved client state is never discarded without consent. The banner SHALL NOT be a modal overlay that blocks interaction — the user MUST be able to continue working and reload when ready.

All banner copy (the "new version available" message and the reload control label) SHALL be localised via the project's paraglide message system, with keys defined in every supported locale file.

This requirement covers *frontend bundle staleness only*. It SHALL NOT be driven by the backend `/api/health` version or commit — the frontend and backend are separately versioned/deployed artifacts, so backend drift is out of scope for this prompt.

#### Scenario: A newer deployed version triggers the banner
- **GIVEN** a tab running UI build version `A`
- **WHEN** a newer build version `B` has been deployed and SvelteKit's poll observes `B`
- **THEN** `updated.current` becomes `true` and the `UpdateBanner` renders with the localised "new version available" message and a reload control

#### Scenario: Up-to-date tab shows no banner
- **GIVEN** a tab running the currently deployed UI build version
- **WHEN** the version poll runs
- **THEN** `updated.current` is `false` and the `UpdateBanner` does not render

#### Scenario: Reload is user-initiated and not silent
- **GIVEN** the `UpdateBanner` is visible
- **WHEN** the user does NOT activate the reload control
- **THEN** the app does not reload, and the user can continue interacting with the current page
- **AND WHEN** the user activates the reload control
- **THEN** `location.reload()` is invoked, fetching the new bundle

#### Scenario: Local development does not spuriously prompt
- **GIVEN** a local build with no `APP_VERSION` env set
- **WHEN** the app is built and run
- **THEN** `version.name` uses the fixed dev fallback string and the banner does not appear solely due to local rebuilds

#### Scenario: Banner copy is localised
- **WHEN** the `UpdateBanner` renders for a user whose locale is German
- **THEN** the message and reload control are rendered from the German paraglide messages (not hard-coded English)
