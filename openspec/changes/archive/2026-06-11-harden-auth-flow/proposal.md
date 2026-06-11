# Harden Auth Flow

## Why

A backend security review (2026-06-11) found four exploitable weaknesses in the SSO login flow and session-cookie handling: the OAuth `state` value is not bound to the browser that started the login (classic login-CSRF / session fixation), the in-memory in-flight OAuth store grows without bound under `/auth/login` spam, the session cookie is issued without the `Secure` attribute, and `/auth/logout` is a state-changing GET reachable cross-site under `SameSite=Lax`.

## What Changes

- Bind the OAuth `state` to the initiating browser: `/auth/login` (and `/auth/characters/add`) set a short-lived `HttpOnly` state cookie; `/auth/callback` requires the cookie to match the `state` query parameter before consuming the in-flight record, and clears it afterwards.
- Evict in-flight OAuth records: each `InflightRecord` gets a creation timestamp and a TTL (15 minutes); expired records are rejected at the callback and swept opportunistically on insert. The store is capped; when full, the oldest expired entries are evicted first and new logins are refused only if the cap is hit with no expired entries to evict.
- Add `Secure` to the session cookie (set and clear paths) and to the new state cookie. Local development behind plain HTTP is handled per design.md.
- **BREAKING** Convert `/auth/logout` from GET to POST so a cross-site top-level navigation (or browser prefetch) can no longer end a session. The frontend logout control becomes a form submission.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `eve-sso-auth`: the login/callback contract gains the browser-bound state cookie requirement and in-flight record expiry; the logout method changes from GET to POST; the session-cookie requirement gains the `Secure` attribute (covers issue, refresh, and clear paths).

## Impact

- Backend: `handlers/auth.rs` (login, callback, add_character, logout), `handlers/cookie.rs` (new state cookie helpers + `Secure`), `session.rs` (`InflightStore` TTL/cap), `lib.rs` (logout route method).
- Frontend: logout link becomes a `<form method="POST">` (GlobalNav and anywhere else `/auth/logout` is linked); e2e specs that drive logout.
- Tests: HURL session/auth files asserting cookie attributes and the logout method; integration tests for callback state-cookie mismatch and expired in-flight records.
- No database changes. No new dependencies.
