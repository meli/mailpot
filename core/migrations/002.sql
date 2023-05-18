PRAGMA foreign_keys=ON;
ALTER TABLE list ADD COLUMN topics JSON NOT NULL CHECK (json_type(topics) == 'array') DEFAULT '[]';
