CREATE TABLE llm_route (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    provider TEXT NOT NULL DEFAULT 'auto',
    model TEXT NOT NULL DEFAULT 'auto',
    updated_at TEXT NOT NULL
);
