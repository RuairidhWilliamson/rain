ALTER TABLE sessions ADD COLUMN created_at TIMESTAMP NULL;
UPDATE sessions SET created_at=expires_at;
ALTER TABLE sessions ALTER COLUMN created_at SET NOT NULL;
