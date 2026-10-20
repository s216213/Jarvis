CREATE TABLE client_launches (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    app TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX client_launches_user_created_idx ON client_launches (user_id, created_at DESC);

CREATE TABLE client_launch_acks (
    launch_id TEXT NOT NULL REFERENCES client_launches(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    acked_at TEXT NOT NULL,
    PRIMARY KEY (launch_id, device_id)
);
