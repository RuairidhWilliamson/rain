CREATE TABLE users (
    id BIGINT PRIMARY KEY,
    login TEXT NOT NULL,
    name TEXT NOT NULL,
    avatar_url TEXT NOT NULL
);

CREATE TABLE sessions (
    id UUID PRIMARY KEY,
    csrf TEXT,
    user_id BIGINT REFERENCES users,
    expires_at TIMESTAMP NOT NULL
);

CREATE TYPE "RunSource" AS ENUM (
    'Github'
);

CREATE TYPE "RunStatus" AS ENUM (
    'Success',
    'Failure'
);

CREATE TABLE runs (
    id BIGSERIAL PRIMARY KEY,
    source "RunSource" NOT NULL,
    created_at TIMESTAMP NOT NULL,
    repo_owner TEXT NOT NULL,
    repo_name TEXT NOT NULL,
    commit TEXT NOT NULL,
    dequeued_at TIMESTAMP
);

CREATE TABLE finished_runs (
    run BIGSERIAL REFERENCES runs,
    finished_at TIMESTAMP NOT NULL,
    status "RunStatus" NOT NULL,
    execution_time_millis BIGINT NOT NULL,
    output TEXT NOT NULL
);
