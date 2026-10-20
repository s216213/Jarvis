CREATE TABLE client_launches (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    app TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX client_launches_user_created_idx ON client_launches (user_id, created_at DESC);

CREATE TABLE client_launch_acks (
    launch_id UUID NOT NULL REFERENCES client_launches(id) ON DELETE CASCADE,
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    acked_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (launch_id, device_id)
);
