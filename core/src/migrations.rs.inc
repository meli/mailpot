
//(user_version, redo sql, undo sql
&[(1,"PRAGMA foreign_keys=ON;
ALTER TABLE templates RENAME TO template;
","PRAGMA foreign_keys=ON;
ALTER TABLE template RENAME TO templates;
"),(2,"PRAGMA foreign_keys=ON;
ALTER TABLE list ADD COLUMN topics JSON NOT NULL CHECK (json_type(topics) == 'array') DEFAULT '[]';
","PRAGMA foreign_keys=ON;
ALTER TABLE list DROP COLUMN topics;
"),(3,"PRAGMA foreign_keys=ON;

UPDATE list SET topics = arr FROM (SELECT json_group_array(ord.val) AS arr, ord.pk AS pk FROM (SELECT json_each.value AS val, list.pk AS pk FROM list, json_each(list.topics) ORDER BY val ASC) AS ord GROUP BY pk) AS ord WHERE ord.pk = list.pk;

CREATE TRIGGER
IF NOT EXISTS sort_topics_update_trigger
AFTER UPDATE ON list
FOR EACH ROW
WHEN NEW.topics != OLD.topics
BEGIN
  UPDATE list SET topics = ord.arr FROM (SELECT json_group_array(ord.val) AS arr, ord.pk AS pk FROM (SELECT json_each.value AS val, list.pk AS pk FROM list, json_each(list.topics) ORDER BY val ASC) AS ord GROUP BY pk) AS ord WHERE ord.pk = list.pk AND list.pk = NEW.pk;
END;

CREATE TRIGGER
IF NOT EXISTS sort_topics_new_trigger
AFTER INSERT ON list
FOR EACH ROW
BEGIN
  UPDATE list SET topics = arr FROM (SELECT json_group_array(ord.val) AS arr, ord.pk AS pk FROM (SELECT json_each.value AS val, list.pk AS pk FROM list, json_each(list.topics) ORDER BY val ASC) AS ord GROUP BY pk) AS ord WHERE ord.pk = list.pk AND list.pk = NEW.pk;
END;
","PRAGMA foreign_keys=ON;

DROP TRIGGER sort_topics_update_trigger;
DROP TRIGGER sort_topics_new_trigger;
"),]