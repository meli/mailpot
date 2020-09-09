-- Your SQL goes here

PRAGMA foreign_keys = false;
ALTER TABLE membership ADD COLUMN enabled BOOLEAN CHECK (enabled in (0, 1)) NOT NULL DEFAULT 1;
PRAGMA foreign_keys = true;
