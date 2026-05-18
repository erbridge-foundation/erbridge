## ADDED Requirements

This capability defines the `/about` page in the SvelteKit frontend, its content, and how it is reached from the rest of the UI. The page is publicly reachable — it does NOT require an authenticated session — and is the single user-visible surface for the project's version information, source code link, legal disclaimer, and acknowledgements.

### Requirement: /about route is publicly reachable

The frontend SHALL serve a route at `/about`. The route SHALL be reachable without an authenticated session — `+layout.server.ts`'s redirect-to-`/login` allowlist SHALL include `/about` alongside `/login`. An authenticated user visiting `/about` SHALL see the same content as an unauthenticated visitor (the page is invariant of identity).

#### Scenario: Unauthenticated visitor reaches /about
- **WHEN** a visitor with no session cookie navigates to `/about`
- **THEN** the page renders normally (no redirect to `/login`)

#### Scenario: Authenticated visitor reaches /about
- **WHEN** a visitor with a valid session cookie navigates to `/about`
- **THEN** the page renders the same content as for an unauthenticated visitor

### Requirement: /about is linked from the user-menu dropdown

The user-menu dropdown (`UserMenu.svelte`) SHALL include an `about` link that navigates to `/about`. The link SHALL be positioned **above** the `preferences` and `settings` placeholders, with the existing divider remaining above `log out`. The `about` link SHALL be a real, enabled `<a href="/about">` — not a greyed-out placeholder.

The user-menu is only visible to authenticated users (it is part of the GlobalNav user chip). Unauthenticated users reach `/about` by direct URL or external link.

#### Scenario: about link present in user-menu
- **WHEN** an authenticated user opens the user-menu dropdown
- **THEN** the first item is `about` with `href="/about"`, fully enabled (not `aria-disabled`)

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

The page SHALL include an **Acknowledgements** section with a short list of inspirations. Each entry SHALL be a project name, an external link (opening in a new tab with `rel="noopener noreferrer"`), and a one-line description. At minimum the section SHALL include the following three entries:

- **Tripwire** — https://tripwire.eve-apps.com/ — "the wormhole-mapping reference for a generation of W-space pilots; pioneered the chain-aware signature workflow."
- **Wanderer** — https://wanderer.ltd/ — "modern, open-source, multi-character mapping with strong real-time semantics."
- **Anokis.info** — https://anokis.info/ — "the institutional encyclopedia of W-space; the static-info source the community has trusted for years."

The acknowledgements list is curated by the project maintainers and lives in the Svelte component (or wireframe); editing it is a code change.

#### Scenario: All three acknowledgements are present
- **WHEN** the page renders
- **THEN** the page contains an `<a href="https://tripwire.eve-apps.com/">`, an `<a href="https://wanderer.ltd/">`, and an `<a href="https://anokis.info/">`, each with `target="_blank"` and `rel="noopener noreferrer"`
