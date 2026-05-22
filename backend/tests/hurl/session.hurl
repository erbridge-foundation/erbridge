# Session-cookie reissue on a protected request.
#
# Asserts that every cookie-authenticated request returns a fresh `Set-Cookie`
# header carrying a newly-signed session JWT, so the browser-side cookie
# lifetime tracks the server-side session row's `expires_at`.
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 \
#               --variable session=<jwt-from-cookie> \
#               session.hurl
#
# Note: this file intentionally uses a session cookie (not an API key) because
# it is testing the cookie-reissue behaviour of the session middleware.

# ── 1. Cookie-auth GET reissues the session cookie ────────────────────────────

GET {{base_url}}/api/v1/me
Cookie: session={{session}}

HTTP 200
[Asserts]
header "Set-Cookie" matches "^session=[A-Za-z0-9_\\-]+\\.[A-Za-z0-9_\\-]+\\.[A-Za-z0-9_\\-]+;"
header "Set-Cookie" contains "HttpOnly"
header "Set-Cookie" contains "SameSite=Lax"
header "Set-Cookie" contains "Path=/"

# ── 2. Unauthenticated request does NOT reissue a cookie ──────────────────────

GET {{base_url}}/api/v1/me

HTTP 401
[Asserts]
header "Set-Cookie" not exists
