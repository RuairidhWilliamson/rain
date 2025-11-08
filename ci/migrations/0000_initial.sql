CREATE TABLE users (
    id BIGINT PRIMARY KEY,
    login TEXT NOT NULL,
    name TEXT NOT NULL,
    avatar_url TEXT NOT NULL
);

CREATE TABLE sessions (
    id UUID PRIMARY KEY,
    csrf TEXT NULl,
    user_id BIGINT NULL REFERENCES users
);

CREATE TYPE "RunSource" AS ENUM (
    'Github'
);

CREATE TABLE runs (
    id UUID PRIMARY KEY,
    source "RunSource" NOT NULL,
    created_at TIMESTAMP NOT NULL
);
