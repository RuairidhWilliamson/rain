CREATE TABLE secrets (
    repo BIGINT REFERENCES repos,
    name TEXT NOT NULL,
    value TEXT NOT NULL
);
