CREATE TABLE IF NOT EXISTS settings_json_schema (
  pk               INTEGER PRIMARY KEY NOT NULL,
  id               TEXT NOT NULL UNIQUE,
  value            JSON NOT NULL CHECK (json_type(value) = 'object'),
  created          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified    INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS list_settings_json (
  pk               INTEGER PRIMARY KEY NOT NULL,
  name             TEXT NOT NULL,
  list             INTEGER,
  value            JSON NOT NULL CHECK (json_type(value) = 'object'),
  is_valid         BOOLEAN CHECK (is_valid IN (0, 1)) NOT NULL DEFAULT 0, -- BOOLEAN_FALSE-> 0, BOOLEAN_TRUE-> 1
  created          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified    INTEGER NOT NULL DEFAULT (unixepoch()),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE,
  FOREIGN KEY (name) REFERENCES settings_json_schema(id) ON DELETE CASCADE,
  UNIQUE (list, name) ON CONFLICT ROLLBACK
);

CREATE TRIGGER
IF NOT EXISTS is_valid_settings_json_on_update
AFTER UPDATE OF value, name, is_valid ON list_settings_json
FOR EACH ROW
BEGIN
  SELECT RAISE(ROLLBACK, 'new settings value is not valid according to the json schema. Rolling back transaction.') FROM settings_json_schema AS schema WHERE schema.id = NEW.name AND NOT validate_json_schema(schema.value, NEW.value);
  UPDATE list_settings_json SET is_valid = 1 WHERE pk = NEW.pk;
END;

CREATE TRIGGER
IF NOT EXISTS is_valid_settings_json_on_insert
AFTER INSERT ON list_settings_json
FOR EACH ROW
BEGIN
  SELECT RAISE(ROLLBACK, 'new settings value is not valid according to the json schema. Rolling back transaction.') FROM settings_json_schema AS schema WHERE schema.id = NEW.name AND NOT validate_json_schema(schema.value, NEW.value);
  UPDATE list_settings_json SET is_valid = 1 WHERE pk = NEW.pk;
END;

CREATE TRIGGER
IF NOT EXISTS invalidate_settings_json_on_schema_update
AFTER UPDATE OF value, id ON settings_json_schema
FOR EACH ROW
BEGIN
  UPDATE list_settings_json SET name = NEW.id, is_valid = 0 WHERE name = OLD.id;
END;

DROP TRIGGER IF EXISTS last_modified_list;
DROP TRIGGER IF EXISTS last_modified_owner;
DROP TRIGGER IF EXISTS last_modified_post_policy;
DROP TRIGGER IF EXISTS last_modified_subscription_policy;
DROP TRIGGER IF EXISTS last_modified_subscription;
DROP TRIGGER IF EXISTS last_modified_account;
DROP TRIGGER IF EXISTS last_modified_candidate_subscription;
DROP TRIGGER IF EXISTS last_modified_template;
DROP TRIGGER IF EXISTS last_modified_settings_json_schema;
DROP TRIGGER IF EXISTS last_modified_list_settings_json;

-- [tag:last_modified_list]: update last_modified on every change.
CREATE TRIGGER
IF NOT EXISTS last_modified_list
AFTER UPDATE ON list
FOR EACH ROW
WHEN NEW.last_modified == OLD.last_modified
BEGIN
  UPDATE list SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

-- [tag:last_modified_owner]: update last_modified on every change.
CREATE TRIGGER
IF NOT EXISTS last_modified_owner
AFTER UPDATE ON owner
FOR EACH ROW
WHEN NEW.last_modified == OLD.last_modified
BEGIN
  UPDATE owner SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

-- [tag:last_modified_post_policy]: update last_modified on every change.
CREATE TRIGGER
IF NOT EXISTS last_modified_post_policy
AFTER UPDATE ON post_policy
FOR EACH ROW
WHEN NEW.last_modified == OLD.last_modified
BEGIN
  UPDATE post_policy SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

-- [tag:last_modified_subscription_policy]: update last_modified on every change.
CREATE TRIGGER
IF NOT EXISTS last_modified_subscription_policy
AFTER UPDATE ON subscription_policy
FOR EACH ROW
WHEN NEW.last_modified == OLD.last_modified
BEGIN
  UPDATE subscription_policy SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

-- [tag:last_modified_subscription]: update last_modified on every change.
CREATE TRIGGER
IF NOT EXISTS last_modified_subscription
AFTER UPDATE ON subscription
FOR EACH ROW
WHEN NEW.last_modified == OLD.last_modified
BEGIN
  UPDATE subscription SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

-- [tag:last_modified_account]: update last_modified on every change.
CREATE TRIGGER
IF NOT EXISTS last_modified_account
AFTER UPDATE ON account
FOR EACH ROW
WHEN NEW.last_modified == OLD.last_modified
BEGIN
  UPDATE account SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

-- [tag:last_modified_candidate_subscription]: update last_modified on every change.
CREATE TRIGGER
IF NOT EXISTS last_modified_candidate_subscription
AFTER UPDATE ON candidate_subscription
FOR EACH ROW
WHEN NEW.last_modified == OLD.last_modified
BEGIN
  UPDATE candidate_subscription SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

-- [tag:last_modified_template]: update last_modified on every change.
CREATE TRIGGER
IF NOT EXISTS last_modified_template
AFTER UPDATE ON template
FOR EACH ROW
WHEN NEW.last_modified == OLD.last_modified
BEGIN
  UPDATE template SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

-- [tag:last_modified_settings_json_schema]: update last_modified on every change.
CREATE TRIGGER
IF NOT EXISTS last_modified_settings_json_schema
AFTER UPDATE ON settings_json_schema
FOR EACH ROW
WHEN NEW.last_modified == OLD.last_modified
BEGIN
  UPDATE settings_json_schema SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

-- [tag:last_modified_list_settings_json]: update last_modified on every change.
CREATE TRIGGER
IF NOT EXISTS last_modified_list_settings_json
AFTER UPDATE ON list_settings_json
FOR EACH ROW
WHEN NEW.last_modified == OLD.last_modified
BEGIN
  UPDATE list_settings_json SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;
