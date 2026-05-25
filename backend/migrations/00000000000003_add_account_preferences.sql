-- User preferences (accessibility, locale, …) stored as a JSONB bag on the
-- account. A bag rather than typed columns so new preference keys can be added
-- without a migration; the application (service layer) validates keys/values.
-- Existing accounts default to {} meaning "all preferences at their default".
ALTER TABLE account
    ADD COLUMN preferences JSONB NOT NULL DEFAULT '{}'::jsonb;
