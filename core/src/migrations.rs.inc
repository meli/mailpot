
//(user_version, redo sql, undo sql
&[(1,"PRAGMA foreign_keys=ON;
BEGIN;
ALTER TABLE templates RENAME TO template;
COMMIT;
","PRAGMA foreign_keys=ON;
BEGIN;
ALTER TABLE template RENAME TO templates;
COMMIT;
"),]