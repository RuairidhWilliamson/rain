CREATE TABLE users (
    id BIGINT PRIMARY KEY,
    login TEXT NOT NULL,
    name TEXT NOT NULL,
    avatar_url TEXT NOT NULL
);

CREATE TABLE sessions (
    id UUID PRIMARY KEY,
    csrf TEXT,
    user_id BIGINT REFERENCES users
);

CREATE TYPE "RunSource" AS ENUM (
    'Github'
);

CREATE TYPE "RunState" AS ENUM (
    'Queued',
    'InProgress',
    'Finished'
);

CREATE TYPE "RunStatus" AS ENUM (
    'Success',
    'Failure',
);

CREATE TABLE runs (
    id UUID PRIMARY KEY,
    source "RunSource" NOT NULL,
    created_at TIMESTAMP NOT NULL,
    state "RunState" NOT NULL,
    status "RunStatus"
);
