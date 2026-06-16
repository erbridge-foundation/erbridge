## MODIFIED Requirements

### Requirement: Login redirects to EVE SSO
The system SHALL redirect the browser to the EVE ESI authorization endpoint when `GET /auth/login` is requested. The authorization URL SHALL be derived from the ESI discovery document fetched at startup, never hardcoded. The redirect SHALL include all required OAuth2 parameters: `response_type=code`, `client_id`, `redirect_uri` (the configured callback URL — `ESI_CALLBACK_URL` when set, otherwise `{APP_URL}/auth/callback`), `scope` (the full required scope list), and a `state` parameter for CSRF protection.

The same configured callback URL SHALL be used as `redirect_uri` everywhere the OAuth2 flow constructs it: the `GET /auth/login` redirect, the `GET /auth/characters/add` redirect, and the authorization-code exchange at the ESI token endpoint. These three callsites SHALL resolve the callback URL from a single source so they cannot diverge.

`GET /auth/login` and `GET /auth/characters/add` SHALL accept an OPTIONAL `?return_to=<path>` query parameter. The value SHALL be validated as a same-origin path: it MUST start with a single `/`, MUST NOT start with `//` or `/\\` (which browsers may interpret as a scheme-relative URL), and MUST NOT contain `\r` or `\n`. The validated value SHALL be stashed alongside the CSRF state in the session's in-flight OAuth2 record. The OAuth2 callback handler SHALL redirect the browser to this path on success. If `return_to` is absent or fails validation, the callback SHALL redirect to `/`.

#### Scenario: Unauthenticated user visits login
- **WHEN** a browser requests `GET /auth/login`
- **THEN** the backend responds with HTTP 302 redirecting to the EVE SSO authorization URL with all required query parameters

#### Scenario: Authorization URL is not hardcoded
- **WHEN** the backend starts up
- **THEN** it fetches `https://login.eveonline.com/.well-known/oauth-authorization-server` and uses `authorization_endpoint` from the response for all login redirects

#### Scenario: redirect_uri defaults to APP_URL-derived callback
- **WHEN** `ESI_CALLBACK_URL` is not set and the login redirect is built
- **THEN** the `redirect_uri` parameter is `{APP_URL}/auth/callback`

#### Scenario: redirect_uri honours an explicit ESI_CALLBACK_URL
- **WHEN** `ESI_CALLBACK_URL` is set to an explicit value (e.g. a proxied path that differs from `{APP_URL}/auth/callback`)
- **THEN** the login redirect, the add-character redirect, and the token-exchange request all send that exact value as `redirect_uri`
