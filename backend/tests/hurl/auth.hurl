# SSO auth-flow HTTP contract (harden-auth-flow).
#
# Covers the browser-bound OAuth state cookie, the callback's state-cookie
# rejection paths, and the POST-only logout method. No SSO round-trip is
# performed: the callback assertions fail before any token exchange, and logout
# without a session is a plain redirect.
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 \
#               --location-trusted=false \
#               auth.hurl
#
# Note: requests use --location=false implicitly (hurl does not follow 3xx in
# --test mode unless --location is passed), so the 302s below are asserted as-is.

# ── 1. /auth/login sets the browser-bound auth_state cookie ───────────────────

GET {{base_url}}/auth/login

HTTP 302
[Asserts]
header "Set-Cookie" contains "auth_state="
header "Set-Cookie" contains "HttpOnly"
header "Set-Cookie" contains "SameSite=Lax"
header "Set-Cookie" contains "Secure"
header "Set-Cookie" contains "Path=/auth"
header "Set-Cookie" contains "Max-Age=900"

# ── 2. /auth/callback with no auth_state cookie is rejected (no token exchange) ─

GET {{base_url}}/auth/callback?code=abc&state=some-state

HTTP 400

# ── 3. /auth/callback with a mismatching auth_state cookie is rejected ────────

GET {{base_url}}/auth/callback?code=abc&state=some-state
Cookie: auth_state=a-different-value

HTTP 400
[Asserts]
# The stale cookie is cleared on rejection.
header "Set-Cookie" contains "auth_state="
header "Set-Cookie" contains "Max-Age=0"

# ── 4. GET /auth/logout is refused (state-changing logout must be POST) ───────

GET {{base_url}}/auth/logout

HTTP 405

# ── 5. POST /auth/logout without a session redirects home and clears cookie ───

POST {{base_url}}/auth/logout

HTTP 302
[Asserts]
header "Location" == "/"
header "Set-Cookie" contains "session="
header "Set-Cookie" contains "Max-Age=0"
header "Set-Cookie" contains "Secure"
