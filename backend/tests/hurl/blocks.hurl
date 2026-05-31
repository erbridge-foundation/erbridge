# Block enforcement on the bearer route (account_blocked).
#
# Asserts that once an admin blocks a character owned by an account, that
# account's API key is rejected with 401 `account_blocked` (the key row is NOT
# deleted — it simply no longer authenticates). Mirrors account.hurl's
# soft-delete rejection, for the block axis.
#
# Prerequisites:
#   --secret   erb_api_key=<key of the VICTIM account being blocked>
#   --variable admin_session=<session JWT of a SERVER-ADMIN account>
#   --variable victim_eve_character_id=<an eve_character_id owned by the key's account>
#
# The admin and victim MUST be different accounts (an admin cannot block their
# own character — that is cannot_block_self).
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 \
#               --variable admin_session=$ADMIN_SESSION \
#               --variable victim_eve_character_id=$VICTIM_CHAR_ID \
#               --secret    erb_api_key=$VICTIM_API_KEY \
#               blocks.hurl
#
# WARNING: this blocks the victim character. Unblock it afterwards (see the
# final step, which restores state) — note the victim's tokens/sessions are
# cleared by the block and are NOT restored on unblock (re-SSO required).

# ── 1. Victim's key works before the block → 200 ─────────────────────────────

GET {{base_url}}/api/v1/me
Authorization: Bearer {{erb_api_key}}

HTTP 200
[Asserts]
jsonpath "$.data.account.status" == "active"

# ── 2. Admin blocks the victim's character → 204 ─────────────────────────────

POST {{base_url}}/api/v1/admin/blocks
Content-Type: application/json
Cookie: session={{admin_session}}
{"eve_character_id": {{victim_eve_character_id}}, "reason": "hurl-block-enforcement"}

HTTP 204

# ── 3. Victim's key is now rejected → 401 account_blocked ────────────────────

GET {{base_url}}/api/v1/me
Authorization: Bearer {{erb_api_key}}

HTTP 401
[Asserts]
jsonpath "$.error.code" == "account_blocked"

# ── 4. Restore state: unblock the victim character → 204 ─────────────────────
# (Tokens/sessions are not restored; the victim must re-SSO to use the service.)

DELETE {{base_url}}/api/v1/admin/blocks/{{victim_eve_character_id}}
Cookie: session={{admin_session}}

HTTP 204
