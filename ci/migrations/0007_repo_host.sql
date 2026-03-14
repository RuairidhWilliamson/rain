CREATE TABLE repo_hosts (
    id BIGSERIAL PRIMARY KEY,
    kind TEXT NOT NULL,
    url TEXT NOT NULL,
    app_id TEXT,
    app_key TEXT,
    webhook_secret TEXT
);

-- Migrate existing repo hosts into the new table, they won't have credentials but at least there is no data loss
INSERT INTO repo_hosts (kind, url) SELECT DISTINCT host_api, host_url FROM repos;
ALTER TABLE repos ADD COLUMN host BIGINT NULL REFERENCES repo_hosts;
UPDATE repos SET host=repo_hosts.id FROM repo_hosts WHERE host_api=kind AND host_url=url;
ALTER TABLE repos ALTER COLUMN host SET NOT NULL;
ALTER TABLE repos DROP COLUMN host_api;
ALTER TABLE repos DROP COLUMN host_url;
