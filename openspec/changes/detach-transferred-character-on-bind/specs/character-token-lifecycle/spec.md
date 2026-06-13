## ADDED Requirements

### Requirement: Owner-hash change is acted on at bind time, not only by the sweep

A change in a character's `owner` hash is CCP's canonical transfer signal. In addition to the daily refresh sweep's passive detection (which flags `token_status = owner_mismatch` on a successful refresh whose hash differs), the SSO bind path SHALL act on an owner-hash change at authentication time: when a character authenticates whose presented `owner` hash differs from a non-null stored `owner_hash`, the bind path SHALL detach the character from its prior account and rebind it to the authenticating owner, per the `eve-sso-auth` capability.

This bind-time action and the sweep's flagging are complementary, not contradictory: the sweep flags a transfer it observes on a background refresh of a character still bound to its old account; the bind-time action resolves the transfer the moment the new owner authenticates. A bind-time detach-and-rebind makes the stale `owner_mismatch` flag moot because the character is re-homed and re-stamped `token_status = valid` against the presented (current) hash.

The transfer predicate at bind time SHALL be identical in spirit to the sweep's: a differing hash counts only against a **non-null** stored hash. A null stored hash or an absent presented hash SHALL NOT be treated as a transfer (the current authentication merely records the hash for future comparison).

#### Scenario: Bind-time transfer re-homes the character and clears the stale flag
- **WHEN** a character previously flagged `owner_mismatch` (or still `valid`) authenticates with an `owner` hash differing from its non-null stored hash, while bound to another account
- **THEN** the bind path detaches and rebinds it to the authenticating owner and sets `token_status = valid` against the presented hash, per the `eve-sso-auth` capability

#### Scenario: Null stored hash at bind time is not a transfer
- **WHEN** a character whose stored `owner_hash IS NULL` authenticates
- **THEN** the presented hash is recorded and no detach/transfer occurs

#### Scenario: Sweep flagging remains for unbound-at-rest transfers
- **WHEN** the daily sweep refreshes a character still bound to its old account and observes a differing hash
- **THEN** it still sets `token_status = owner_mismatch` per the existing sweep behaviour (the new owner has not yet authenticated to trigger bind-time re-homing)
