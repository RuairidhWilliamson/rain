CREATE TABLE repos (
    id BIGSERIAL PRIMARY KEY,
    host TEXT NOT NULL,
    owner TEXT NOT NULL,
    name TEXT NOT NULL
);

INSERT INTO repos (host, owner, name) SELECT DISTINCT source, repo_owner, repo_name FROM runs;

ALTER TABLE runs ADD COLUMN repo BIGINT REFERENCES repos;

UPDATE runs SET repo=repos.id FROM repos WHERE repos.host=runs.source AND repos.owner=runs.repo_owner AND repos.name=runs.repo_name;

ALTER TABLE runs ALTER COLUMN repo SET NOT NULL;

ALTER TABLE runs DROP COLUMN source;
ALTER TABLE runs DROP COLUMN repo_owner;
ALTER TABLE runs DROP COLUMN repo_name;
