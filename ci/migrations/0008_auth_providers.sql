CREATE TABLE auth_providers (
    name TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    oidc_discovery_url TEXT,
    client_id TEXT NOT NULL,
    client_secret TEXT NOT NULL
);

ALTER TABLE users ADD COLUMN provider TEXT REFERENCES auth_providers;
ALTER TABLE users ADD COLUMN email TEXT;
ALTER TABLE users ALTER COLUMN avatar_url DROP NOT NULL;
ALTER TABLE users ALTER COLUMN login DROP NOT NULL;
ALTER TABLE users ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY;
