ALTER TABLE client_launches ADD COLUMN device_id TEXT REFERENCES devices(id) ON DELETE CASCADE;

CREATE TABLE device_handoffs (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX device_handoffs_user_created_idx ON device_handoffs (user_id, created_at DESC);

CREATE TABLE device_handoff_acks (
    handoff_id TEXT NOT NULL REFERENCES device_handoffs(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    acked_at TEXT NOT NULL,
    PRIMARY KEY (handoff_id, device_id)
);
