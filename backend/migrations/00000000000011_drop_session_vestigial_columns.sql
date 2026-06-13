-- Drop the vestigial session columns. Nothing reads either: CSRF state lives in
-- the in-memory in-flight store and (since harden-auth-flow) the auth_state
-- cookie, and add-character mode is decided entirely from the in-flight record.
-- Both columns were write-only.
ALTER TABLE session
    DROP COLUMN csrf_state,
    DROP COLUMN add_character_mode;
