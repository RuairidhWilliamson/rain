ALTER TABLE repos RENAME COLUMN host TO host_api;
ALTER TABLE repos ADD COLUMN host_url TEXT;
UPDATE repos SET host_url='https://github.com' WHERE host_url IS NULL;
ALTER TABLE repos ALTER COLUMN host_url SET NOT NULL;
