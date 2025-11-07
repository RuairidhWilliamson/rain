CREATE TABLE IF NOT EXISTS users (
    id BIGINT PRIMARY KEY,
    login TEXT NOT NULL,
    name TEXT NOT NULL,
    avatar_url TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY,
    csrf TEXT NULl,
    user_id BIGINT NULL REFERENCES users
);
