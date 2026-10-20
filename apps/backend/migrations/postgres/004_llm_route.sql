CREATE TABLE llm_route (
    id SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    provider TEXT NOT NULL DEFAULT 'auto',
    model TEXT NOT NULL DEFAULT 'auto',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
