ALTER TABLE client_launches ADD COLUMN device_id UUID REFERENCES devices(id) ON DELETE CASCADE;

CREATE TABLE device_handoffs (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX device_handoffs_user_created_idx ON device_handoffs (user_id, created_at DESC);

CREATE TABLE device_handoff_acks (
    handoff_id UUID NOT NULL REFERENCES device_handoffs(id) ON DELETE CASCADE,
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    acked_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (handoff_id, device_id)
);
