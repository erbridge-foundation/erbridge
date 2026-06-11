## MODIFIED Requirements

### Requirement: list_audit_log reads newest-first with filter axes, a name search, a time window, and a keyset cursor

The system SHALL provide an async function `audit::list_audit_log` returning `Vec<AuditLogEntry>` where each `AuditLogEntry` carries `id`, `occurred_at`, `actor_account_id`, `actor_character_id`, `actor_character_name`, `event_type`, `details`, `target_type`, `target_id`, and `target_name`.

The function SHALL accept these optional filter axes plus a name search, a time window, a keyset cursor, and a limit, all bound as parameters (no string interpolation, no SQL injection surface):

- `event_type: Option<&str>` — if `Some`, restricts to rows where `event_type` matches; if `None`, no restriction.
- `actor_account_id: Option<Uuid>` — if `Some`, restricts to rows where `actor_account_id` matches; if `None`, no restriction.
- `target_type: Option<&str>` — if `Some`, restricts to rows where `target_type` matches; if `None`, no restriction.
- `target_id: Option<&str>` — if `Some`, restricts to rows where `target_id` matches; if `None`, no restriction.
- `target_name: Option<&str>` — if `Some`, restricts to rows where `target_name` contains the argument as a case-insensitive substring (`target_name ILIKE '%fragment%'`); LIKE metacharacters (`%`, `_`, `\`) in the argument SHALL be escaped so they match literally. If `None`, no restriction.
- `q: Option<&str>` — the combined name-search axis. If `Some`, restricts to rows where **either** `actor_character_name` **or** `target_name` contains the argument as a case-insensitive substring (`(actor_character_name ILIKE '%fragment%' OR target_name ILIKE '%fragment%')`); LIKE metacharacters in the argument SHALL be escaped so they match literally, using the same escaping as `target_name`. If `None`, no restriction. `q` and `target_name` MAY both be supplied; they then combine conjunctively (a row must satisfy both).
- `since: Option<DateTime<Utc>>` — if `Some`, restricts to rows where `occurred_at >= since` (the lower bound of the time window); if `None`, no lower restriction.
- `before: Option<DateTime<Utc>>` — if `Some`, restricts to rows where `occurred_at < before` (keyset cursor for newest-first pagination, and the exclusive upper bound of the time window); if `None`, no upper restriction.
- `limit: i64` — maximum rows to return; the caller is responsible for clamping to a sensible upper bound.

Results SHALL be ordered by `occurred_at DESC` (newest first). When multiple axes are supplied they SHALL be combined conjunctively (AND). The `since` lower bound SHALL use the existing `audit_log_occurred_at_idx` for a bounded range scan; the substring axes (`q`, `target_name`) operate over the rows surviving the time bound and other equality filters, which the time window keeps small. No new index and no Postgres extension are required.

#### Scenario: List with no filters returns rows newest-first
- **GIVEN** several `audit_log` rows
- **WHEN** `list_audit_log` is called with all axes `None` and a limit
- **THEN** all rows are returned in descending `occurred_at` order, capped at the `limit`, each carrying its `target_*` columns

#### Scenario: Filter by target_id restricts to that entity's events
- **GIVEN** audit rows targeting several entities
- **WHEN** `list_audit_log` is called with `target_type = Some("character")` and `target_id = Some("555")`
- **THEN** only rows whose `target_type = "character"` and `target_id = "555"` are returned

#### Scenario: q matches either the actor name or the target name
- **GIVEN** an audit row whose `actor_character_name = "Wasp 223"` (target unrelated), and another row whose `target_name = "Red Wasp Industries"` (actor unrelated)
- **WHEN** `list_audit_log` is called with `q = Some("wasp")`
- **THEN** both rows are returned (the first matched on actor name, the second on a substring of the target name)

#### Scenario: q is a case-insensitive substring, not anchored
- **GIVEN** an audit row with `target_name = "The Wasp"`
- **WHEN** `list_audit_log` is called with `q = Some("wasp")`
- **THEN** the row is returned (the fragment matches mid-name, not only as a prefix)

#### Scenario: q LIKE metacharacters are treated literally
- **GIVEN** an audit row with `actor_character_name = "Wasp 223"` and a row with `actor_character_name = "50% Off"`
- **WHEN** `list_audit_log` is called with `q = Some("%")`
- **THEN** the `Wasp 223` row is NOT returned and the `50% Off` row IS returned (the `%` is escaped and matched as a literal character, not a wildcard)

#### Scenario: since bounds the lower edge of the time window
- **GIVEN** audit rows spanning several months
- **WHEN** `list_audit_log` is called with `since = Some(T)` and `before = None`
- **THEN** only rows with `occurred_at >= T` are returned, newest-first

#### Scenario: since and before together express a bounded window
- **GIVEN** audit rows spanning several months
- **WHEN** `list_audit_log` is called with `since = Some(T_lo)` and `before = Some(T_hi)` where `T_lo < T_hi`
- **THEN** only rows with `T_lo <= occurred_at < T_hi` are returned, supporting keyset pagination *within* a fixed window (each page narrows `before`, `since` stays fixed)

#### Scenario: q combines conjunctively with other axes
- **GIVEN** mixed audit rows
- **WHEN** `list_audit_log` is called with `q = Some("wasp")`, `event_type = Some("acl_member_added")`, and `since = Some(T)`
- **THEN** only rows that match the name search AND the event type AND fall within the time window are returned

#### Scenario: Filter by target_name is a case-insensitive substring match
- **GIVEN** an audit row with `target_name = "Wasp 223"`
- **WHEN** `list_audit_log` is called with `target_name = Some("wasp")`
- **THEN** that row is returned (a lowercased fragment matches a substring of the name)

#### Scenario: before cursor advances pagination
- **GIVEN** a previous page's oldest `occurred_at = T`
- **WHEN** `list_audit_log` is called with `before = Some(T)`
- **THEN** only rows with `occurred_at < T` are returned, supporting stable keyset pagination under concurrent inserts
