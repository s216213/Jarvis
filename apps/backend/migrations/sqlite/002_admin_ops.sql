CREATE TABLE devices (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    platform TEXT NOT NULL CHECK (platform IN ('web', 'android', 'desktop', 'unknown')),
    client_version TEXT NOT NULL DEFAULT '',
    fingerprint TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    is_revoked INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    UNIQUE (user_id, fingerprint)
);

CREATE INDEX devices_last_seen_idx ON devices (last_seen_at DESC);

CREATE TABLE feature_flags (
    key TEXT PRIMARY KEY NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    description TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL
);

CREATE TABLE terminal_audit (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    command TEXT NOT NULL,
    exit_code INTEGER,
    created_at TEXT NOT NULL
);

CREATE INDEX terminal_audit_created_idx ON terminal_audit (created_at DESC);
