## ADDED Requirements

### Requirement: is_server_admin from /me gates the admin UI affordance

`GET /api/v1/me` already returns `data.account.is_server_admin` (per the existing `GET /api/v1/me` requirement). The frontend SHALL use that field to decide whether to surface the admin-navigation affordance and whether to attempt admin routes. This requirement introduces no behavioural change to `GET /api/v1/me`; it records that the existing field is the authority for the admin-UI gate, so the gate and the backend's `AdminAccount` extractor agree on a single source of truth.

#### Scenario: Admin field drives the affordance
- **WHEN** `GET /api/v1/me` returns `data.account.is_server_admin = true`
- **THEN** the frontend MAY surface the admin affordance; when it is `false`, the frontend SHALL NOT surface it

#### Scenario: /me itself is unchanged
- **WHEN** any caller fetches `GET /api/v1/me`
- **THEN** the response shape and fields are exactly as defined by the existing `GET /api/v1/me` requirement; this change adds no field and removes none
