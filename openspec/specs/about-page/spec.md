## Purpose

Defines the `/about` page in the SvelteKit frontend, its content, and how it is reached from the rest of the UI. The page is gated — it requires an authenticated session — and is the single user-visible surface for the project's version information, source code link, legal disclaimer, and acknowledgements.

## Requirements

### Requirement: /about route is gated behind authentication

The frontend SHALL serve a route at `/about`, reachable only from the authenticated user-menu. `+layout.server.ts` SHALL NOT treat `/about` as a public route: an unauthenticated visit (a `getMe` 401) SHALL redirect to `/login` like any other gated page. The page's content is invariant of identity — an authenticated user always sees the same about information regardless of their account.

#### Scenario: Unauthenticated visitor is redirected from /about
- **WHEN** a visitor with no session cookie navigates to `/about`
- **THEN** the load redirects to `/login` (303); the about content is not rendered

#### Scenario: Authenticated visitor reaches /about
- **WHEN** a visitor with a valid session cookie navigates to `/about`
- **THEN** the page renders the about content (no redirect away to `/`)

### Requirement: /about is linked from the user-menu dropdown

The user-menu dropdown (`UserMenu.svelte`) SHALL include an `about` link that navigates to `/about`. The link SHALL be positioned **below** the `preferences` and `settings` items and **above** the divider, so the menu order is `preferences`, `settings`, `about`, divider, `log out`. The `about` link SHALL be a real, enabled `<a href="/about">` — not a greyed-out placeholder. Selecting it SHALL close the dropdown.

(Re-ordered by the `accessibility-preferences` change: once `preferences` became a real route it took the top slot, and `about` moved down to sit just above `log out`. This supersedes the original "positioned above the preferences and settings placeholders / first item is about" ordering.)

The user-menu is only visible to authenticated users (it is part of the GlobalNav user chip), and `/about` is the only entry point to the page — unauthenticated visits are redirected to `/login`.

#### Scenario: about link present in user-menu
- **WHEN** an authenticated user opens the user-menu dropdown
- **THEN** an `about` link with `href="/about"` is present, fully enabled (not `aria-disabled`), positioned below `preferences`/`settings` and above the `log out` divider

### Requirement: /about displays UI and API version information

The page SHALL display:

- The **UI version**, sourced from `frontend/package.json`'s `version` field at build time and surfaced to the page via the SvelteKit/Vite build (e.g. `import.meta.env.PUBLIC_UI_VERSION` or equivalent).
- The **API version** and **commit SHA**, sourced from `GET /api/health` at server load time (in `+page.server.ts`).

When `/api/health` is reachable, the API version and commit SHALL be rendered alongside the UI version. When `/api/health` is unreachable (network error, 5xx), the page SHALL still render and display `API: unreachable` (or equivalent) in place of the API version; the rest of the page (links, disclaimer, acknowledgements) SHALL render normally.

#### Scenario: Health endpoint reachable
- **WHEN** the page server-loads and `GET /api/health` returns 200
- **THEN** the page displays the UI version, the API version from `health.version`, and the commit SHA from `health.commit`

#### Scenario: Health endpoint unreachable
- **WHEN** the page server-loads and `GET /api/health` fails (network error or non-2xx)
- **THEN** the page displays the UI version, the literal text `API: unreachable` in place of the API version, and the rest of the page (links, disclaimer, acknowledgements) renders normally

### Requirement: /about displays the GitHub repository link

The page SHALL display a link to the project's source repository at `https://github.com/erbridge-foundation/erbridge`. The link SHALL open in a new tab (`target="_blank"` with `rel="noopener noreferrer"`).

#### Scenario: GitHub link is present
- **WHEN** the page renders
- **THEN** the page contains an `<a>` with `href="https://github.com/erbridge-foundation/erbridge"`, `target="_blank"`, and `rel="noopener noreferrer"`

### Requirement: /about displays the EVE third-party developer legal disclaimer

The page SHALL display the standard CCP-published EVE Online third-party developer disclaimer verbatim. The disclaimer text SHALL contain the substring `"CCP hf."` (used by the integration test as a guard against accidental deletion). The expected text is:

> EVE Online and the EVE logo are the registered trademarks of CCP hf. All rights are reserved worldwide. All other trademarks are the property of their respective owners. EVE Online, the EVE logo, EVE and all associated logos and designs are the intellectual property of CCP hf. All artwork, screenshots, characters, vehicles, storylines, world facts or other recognizable features of the intellectual property relating to these trademarks are likewise the intellectual property of CCP hf. CCP hf. has granted permission to E-R Bridge to use EVE Online and all associated logos and designs for promotional and information purposes on its website but does not endorse, and is not in any way affiliated with, E-R Bridge. CCP is in no way responsible for the content on or functioning of this website, nor can it be liable for any damage arising from the use of this website.

#### Scenario: Disclaimer is present
- **WHEN** the page renders
- **THEN** the page contains the literal substring `"CCP hf."` and the wording above

### Requirement: /about displays an acknowledgements section

The page SHALL include an **Acknowledgements** section with a short list of inspirations. Each entry SHALL be a project name, an external link (opening in a new tab with `rel="noopener noreferrer"`), and a one-line description. At minimum the section SHALL include the following entries:

- **Tripwire** — https://tripwire.eve-apps.com/ — "the wormhole-mapping reference for a generation of W-space pilots; pioneered the chain-aware signature workflow."
- **Wanderer** — https://wanderer.ltd/ — "modern, open-source, multi-character mapping with strong real-time semantics."
- **Anokis.info** — https://anokis.info/ — "the institutional encyclopedia of W-space; the static-info source the community has trusted for years."
- **EVE Scout** — https://www.eve-scout.com/ — "the Signal Cartel community effort that scouts and publicly shares the Thera and Turnur connections — open wormhole intel as a free service."

The acknowledgements list is curated by the project maintainers and lives in the Svelte component (or wireframe); editing it is a code change. The list MAY grow beyond these entries.

#### Scenario: All acknowledgements are present
- **WHEN** the page renders
- **THEN** the page contains an `<a href="https://tripwire.eve-apps.com/">`, an `<a href="https://wanderer.ltd/">`, an `<a href="https://anokis.info/">`, and an `<a href="https://www.eve-scout.com/">`, each with `target="_blank"` and `rel="noopener noreferrer"`
