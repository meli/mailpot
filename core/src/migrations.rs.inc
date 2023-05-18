
//(user_version, redo sql, undo sql
&[(1,"PRAGMA foreign_keys=ON;
ALTER TABLE templates RENAME TO template;
","PRAGMA foreign_keys=ON;
ALTER TABLE template RENAME TO templates;
"),(2,"PRAGMA foreign_keys=ON;
ALTER TABLE list ADD COLUMN topics JSON NOT NULL CHECK (json_type(topics) == 'array') DEFAULT '[]';
","PRAGMA foreign_keys=ON;
ALTER TABLE list DROP COLUMN topics;
"),]