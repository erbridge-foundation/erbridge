-- EVE system reference catalog: the wormhole-type dictionary, the system spine,
-- and per-system statics. Populated by the daily `eve_system_sync` service from
-- eve-scout (/systems, /wormholetypes) and anoikis (wh-statics). Near-immutable
-- reference data; the sync is the only writer.
--
-- `class` / `target_system_class` are plain text (no enum, no CHECK): CCP has
-- added classes over time (c12-c18) and the import is the sole writer, so a
-- constrained domain would be migration churn for no reader benefit.

-- Parent of system_static's static_code FK, so it is created first.
CREATE TABLE wormhole_type (
    identifier          text   PRIMARY KEY,        -- type code, e.g. "Q003"
    type_id             int    NOT NULL,
    target_system_class text   NOT NULL,           -- "ns" / "c5" / "exit"
    max_jump_mass       bigint NOT NULL,
    max_stable_mass     bigint NOT NULL,
    max_stable_time     int    NOT NULL,           -- minutes
    mass_regeneration   bigint NOT NULL,
    possible_static     bool   NOT NULL,
    wandering_only      bool   NOT NULL,
    signature_level     int[]  NOT NULL,           -- [] for K162
    source              text[] NOT NULL            -- ["c1",..] / ["exit"]
);

CREATE TABLE eve_system (
    system_id        bigint  PRIMARY KEY,          -- 30000142 / 31002274
    name             text    NOT NULL,             -- "Jita" / "J172840" (J-code)
    class            text    NOT NULL,             -- "hs" / "c5" / "jove"
    region_id        bigint  NOT NULL,
    region_name      text    NOT NULL,
    security_status  real    NOT NULL,
    jove_observatory bool    NOT NULL DEFAULT false
);

CREATE INDEX eve_system_name_idx ON eve_system (name);

CREATE TABLE system_static (
    system_id   bigint NOT NULL REFERENCES eve_system (system_id) ON DELETE CASCADE,
    static_code text   NOT NULL REFERENCES wormhole_type (identifier),
    PRIMARY KEY (system_id, static_code)
);
