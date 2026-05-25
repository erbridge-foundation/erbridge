# GET /api/health
#
# Run with:
#   hurl --test --variable base_url=http://localhost:5000 health.hurl
#
# Public endpoint (no auth) returning a flat, unenveloped status document.

GET {{base_url}}/api/health

HTTP 200
[Asserts]
jsonpath "$.status" == "ok"
jsonpath "$.version" exists
jsonpath "$.commit" exists
jsonpath "$.components" count == 1
jsonpath "$.components[0].name" == "db"
jsonpath "$.components[0].status" == "ok"
# Proves the response is NOT wrapped in the api-contract envelope.
jsonpath "$.data" not exists
