# eve-system-catalog

## Purpose

A local reference catalog of EVE Online's system topology — every system plus the
wormhole-type dictionary and per-system statics — so the backend can resolve a
system id to its name/region/class and know what wormhole connections a system's
statics spawn. Assembled from three public feeds (eve-scout `/systems` +
`/wormholetypes`, anoikis `wh-statics`) merged on the J-code, refreshed by a daily
background sync that writes gap-free via a single atomic transaction. This
capability only *populates* the catalog; a read API is a separate follow-up.

## Requirements

### Requirement: A local EVE system catalog with wormhole-type dictionary and per-system statics

The backend SHALL maintain a local reference catalog of EVE systems in three tables:

- `eve_system` — one row per EVE system, keyed by `system_id` (the EVE system id). Each row SHALL carry `name`, `class`, `region_id`, `region_name`, `security_status`, and `jove_observatory`. `name` SHALL be indexed. For wormhole systems `name` is the J-code (e.g. `J172840`).
- `wormhole_type` — the wormhole-type dictionary, keyed by the type code `identifier` (e.g. `Q003`). Each row SHALL carry `type_id`, `target_system_class`, `max_jump_mass`, `max_stable_mass`, `max_stable_time` (minutes), `mass_regeneration`, `possible_static`, `wandering_only`, `signature_level`, and `source`.
- `system_static` — a join table of `(system_id, static_code)` with `system_id` referencing `eve_system(system_id)` and `static_code` referencing `wormhole_type(identifier)`; the pair is the primary key. A row records that the system always spawns a static wormhole of that type.

`class` and `target_system_class` SHALL be stored as free text (not a database enum), so new EVE classes require no schema migration.

#### Scenario: A wormhole system resolves to its class, region, and statics
- **WHEN** the catalog has been synced and a consumer reads system `J172840`
- **THEN** `eve_system` yields its `system_id`, `class` (`c5`), region, and security, and `system_static` joined to `wormhole_type` yields each static's target class, max jump mass, max stable mass, and lifetime

#### Scenario: A static code references the wormhole-type dictionary
- **WHEN** a `system_static` row exists with `static_code = 'D845'`
- **THEN** a `wormhole_type` row with `identifier = 'D845'` exists (the FK is satisfied), describing that type's target class and limits

### Requirement: The catalog is assembled from three public sources merged on J-code

The catalog SHALL be built by fetching three sources and merging them:

- eve-scout `GET /v2/public/systems` SHALL populate `eve_system` (the system spine; this feed includes wormhole systems with their class and id).
- eve-scout `GET /v2/public/wormholetypes` SHALL populate `wormhole_type`.
- anoikis `GET https://anoikis.info/data/wh-statics.json` SHALL populate `system_static`, mapping each J-code to its static type codes.

The merge key between the anoikis statics feed and `eve_system` SHALL be the J-code: an anoikis entry's J-code key equals `eve_system.name`. The anoikis fetch SHALL send an explicit, identifying `User-Agent` request header (a good-citizen courtesy that also future-proofs against User-Agent gating; the host does not currently reject the default).

#### Scenario: J-code statics are attached to the matching system
- **WHEN** the anoikis feed contains `"J172840": {"static": ["D845","N062"]}` and `eve_system` contains a row named `J172840`
- **THEN** `system_static` gains a `(system_id_of_J172840, 'D845')` row and a `(system_id_of_J172840, 'N062')` row

#### Scenario: An unmatched static code is skipped without failing the sync
- **WHEN** an anoikis static code has no corresponding `wormhole_type` row (or its J-code has no `eve_system` row)
- **THEN** that single `system_static` row is skipped and logged, and the rest of the sync completes normally

### Requirement: The catalog is refreshed by a daily background sync

A background service SHALL refresh the catalog on a ~24-hour interval. The first refresh SHALL run at process startup (so the catalog populates on boot). A single refresh run's failure SHALL be logged and SHALL NOT terminate the recurring loop. The service is a standalone spawned task; this change SHALL NOT introduce a job registry and SHALL NOT add an admin-triggered manual import.

#### Scenario: The catalog populates on startup and refreshes daily
- **WHEN** the backend starts
- **THEN** a refresh runs immediately, and thereafter approximately every 24 hours

#### Scenario: A failed run does not stop future runs
- **WHEN** a refresh run returns an error
- **THEN** the error is logged and the next scheduled run still fires ~24 hours later

### Requirement: A refresh fetches all sources before writing and commits atomically

A refresh run SHALL fetch all three sources **before** writing anything to the database. Only if all three fetches succeed SHALL it open a single transaction, upsert all three tables within it (writing `wormhole_type` before `system_static` so the foreign key is satisfiable), and commit atomically. The refresh SHALL use upserts (`INSERT … ON CONFLICT DO UPDATE`) and SHALL NOT use `TRUNCATE`, so concurrent readers are never blocked and always observe a complete prior snapshot until the commit. Rebuilding a refresh's `system_static` rows SHALL occur inside the same transaction.

#### Scenario: Readers see a complete snapshot throughout a refresh
- **WHEN** a refresh is writing while another caller reads the catalog
- **THEN** the reader sees the complete previous catalog until the transaction commits, then the complete new catalog — never an empty or partially-written table

#### Scenario: A fetch failure leaves the existing catalog intact
- **WHEN** any of the three source fetches fails
- **THEN** no transaction is opened, nothing is written, and the previously synced catalog is unchanged

#### Scenario: A transaction failure rolls back wholesale
- **WHEN** the database fails partway through the refresh transaction
- **THEN** the entire transaction is rolled back and the previous catalog is unchanged

### Requirement: A sanity floor prevents an implausibly small payload from wiping data

Before opening the transaction, a refresh SHALL reject any source payload that parses successfully but is implausibly small, and SHALL leave the existing catalog untouched. The thresholds SHALL be: at least 5000 systems, at least 50 wormhole types, and at least 1000 static entries.

#### Scenario: An empty-but-valid response aborts the refresh
- **WHEN** a source returns HTTP 200 with a valid but near-empty body (e.g. anoikis returns `{}`)
- **THEN** the sanity floor rejects the run before any write, the run is logged as failed, and the previously synced catalog is unchanged
