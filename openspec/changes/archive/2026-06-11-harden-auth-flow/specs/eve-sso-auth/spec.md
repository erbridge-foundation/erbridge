# eve-sso-auth — delta for harden-auth-flow

## ADDED Requirements

### Requirement: OAuth state is bound to the initiating browser
`GET /auth/login` and `GET /auth/characters/add` SHALL set an `auth_state` cookie carrying the generated CSRF state value, with attributes `HttpOnly; SameSite=Lax; Secure; Path=/auth; Max-Age=900`. `GET /auth/callback` SHALL reject the request with HTTP 400 unless the `auth_state` cookie is present and equals the `state` query parameter, and SHALL perform this check before consuming the in-flight OAuth record. The callback SHALL clear the `auth_state` cookie on every outcome (success, blocked, and error).

#### Scenario: Login sets the state cookie
- **WHEN** a browser requests `GET /auth/login`
- **THEN** the 302 response carries a `Set-Cookie` header for `auth_state` whose value is the same `state` sent to EVE SSO, with `HttpOnly`, `SameSite=Lax`, `Secure`, `Path=/auth`, and `Max-Age=900`

#### Scenario: Callback without the state cookie is rejected
- **WHEN** a request to `GET /auth/callback?code=…&state=<valid-inflight-state>` arrives with no `auth_state` cookie
- **THEN** the backend responds 400, no token exchange is performed, and the in-flight record is not consumed

#### Scenario: Callback with a mismatching state cookie is rejected
- **WHEN** a request to `GET /auth/callback` arrives whose `auth_state` cookie differs from the `state` query parameter
- **THEN** the backend responds 400 and no token exchange is performed

#### Scenario: Successful callback clears the state cookie
- **WHEN** a callback completes successfully in the browser that initiated the login
- **THEN** the response clears the `auth_state` cookie (`Max-Age=0`) alongside setting the session cookie

### Requirement: In-flight OAuth records expire
In-flight OAuth records SHALL expire 15 minutes after creation. The callback SHALL treat an expired record as absent (HTTP 400). The in-flight store SHALL drop expired records opportunistically when new records are inserted and SHALL cap its size at 10 000 records; when the cap is reached and no expired records can be dropped, `GET /auth/login` SHALL refuse the new login attempt with an error response rather than evicting a live record.

#### Scenario: Expired in-flight record is rejected at the callback
- **WHEN** a callback arrives more than 15 minutes after `GET /auth/login` created its in-flight record
- **THEN** the backend responds 400 (invalid or missing state) and no session is created

#### Scenario: Store does not grow without bound
- **WHEN** `GET /auth/login` is requested repeatedly without any callback completing
- **THEN** records older than 15 minutes are dropped as new records are inserted, and the store never holds more than 10 000 records

## MODIFIED Requirements

### Requirement: Logout clears session
The system SHALL handle `POST /auth/logout` by deleting the session row from the Postgres `session` table and clearing the session cookie. The response SHALL redirect to `/`. The route SHALL NOT be registered for GET: a state-changing logout MUST NOT be reachable via top-level navigation (cross-site links, browser prefetch), so `GET /auth/logout` SHALL respond with HTTP 405.

#### Scenario: Authenticated user logs out
- **WHEN** a browser with a valid session cookie submits `POST /auth/logout`
- **THEN** the row in the `session` table for that session ID is deleted, the session cookie is cleared, and the browser is redirected to `/`

#### Scenario: Unauthenticated user requests logout
- **WHEN** a browser with no session cookie submits `POST /auth/logout`
- **THEN** the backend redirects to `/` without error

#### Scenario: GET logout is refused
- **WHEN** a browser requests `GET /auth/logout`
- **THEN** the backend responds 405 and the session is not deleted

### Requirement: Session cookie does not expose token material
The session cookie SHALL contain only an encrypted, signed session ID — no access tokens, refresh tokens, character IDs, or other EVE credential material. The cookie SHALL be `httpOnly`, `SameSite=Lax`, and `Secure`. The `Secure` attribute SHALL be present on every path that sets or clears the session cookie: the SSO callback's initial `Set-Cookie`, the per-request sliding refresh, and the clear on logout and account deletion.

#### Scenario: Session cookie inspection reveals no token data
- **WHEN** a session is established after login
- **THEN** the `Set-Cookie` header value contains only an opaque session ID; access and refresh tokens are absent from all response headers and cookie values

#### Scenario: Session cookie carries Secure on set, refresh, and clear
- **WHEN** the session cookie is set at login, re-issued by the sliding refresh on an authenticated request, or cleared at logout
- **THEN** each `Set-Cookie` header includes the `Secure` attribute
