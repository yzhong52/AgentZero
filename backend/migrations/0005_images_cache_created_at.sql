ALTER TABLE images_cache ADD COLUMN created_at TEXT NOT NULL DEFAULT (datetime('now'));
