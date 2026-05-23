-- Initial schema. See plan.md §SQLite schema.
-- Secrets are NEVER stored here: signing_profiles.secret_ref is a keyring
-- reference, and Apple ID strings are hashed (apple_id_hash) before persistence.

CREATE TABLE devices (
    udid                    TEXT PRIMARY KEY,
    name                    TEXT NOT NULL,
    ios_version             TEXT,
    developer_mode_enabled  INTEGER NOT NULL DEFAULT 0,
    pairing_status          TEXT NOT NULL DEFAULT 'unpaired',
    transport               TEXT NOT NULL DEFAULT 'remote_xpc',
    wifi_eligible           INTEGER NOT NULL DEFAULT 0,
    last_seen               INTEGER
);

CREATE TABLE apps (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    display_name        TEXT NOT NULL,
    bundle_id           TEXT NOT NULL,
    version             TEXT,
    source_ipa_sha256   TEXT NOT NULL,
    source_ipa_path     TEXT NOT NULL,
    icon_path           TEXT
);

CREATE TABLE signing_profiles (
    id                  TEXT PRIMARY KEY,
    signing_method      TEXT NOT NULL,
    apple_id_hash       TEXT,
    team_id             TEXT,
    profile_metadata    TEXT,
    cert_expires_at     INTEGER,
    secret_ref          TEXT
);

CREATE TABLE installations (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    app_id              INTEGER NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
    device_udid         TEXT NOT NULL REFERENCES devices(udid) ON DELETE CASCADE,
    signing_method      TEXT NOT NULL,
    install_ts          INTEGER NOT NULL,
    expiration_ts       INTEGER NOT NULL,
    cert_id             TEXT REFERENCES signing_profiles(id),
    refresh_status      TEXT NOT NULL DEFAULT 'idle',
    trust_prompt_shown  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE apple_quota_events (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    apple_id_hash   TEXT NOT NULL,
    event_type      TEXT NOT NULL CHECK (event_type IN ('device_registered', 'app_id_created')),
    ts              INTEGER NOT NULL
);

CREATE TABLE jobs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    installation_id INTEGER NOT NULL REFERENCES installations(id) ON DELETE CASCADE,
    kind            TEXT NOT NULL CHECK (kind IN ('refresh_profile', 'refresh_cert')),
    next_run        INTEGER,
    last_run        INTEGER,
    retry_count     INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'pending'
);

CREATE TABLE activity_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    ts              INTEGER NOT NULL,
    severity        TEXT NOT NULL,
    operation       TEXT,
    error_category  TEXT,
    message         TEXT
);

CREATE INDEX idx_installations_device ON installations(device_udid);
CREATE INDEX idx_installations_expiration ON installations(expiration_ts);
CREATE INDEX idx_jobs_next_run ON jobs(next_run);
CREATE INDEX idx_quota_lookup ON apple_quota_events(apple_id_hash, event_type, ts);
CREATE INDEX idx_activity_ts ON activity_log(ts);
