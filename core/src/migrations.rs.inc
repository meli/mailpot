
//(user_version, redo sql, undo sql
&[(1,r###"PRAGMA foreign_keys=ON;
ALTER TABLE templates RENAME TO template;"###,r###"PRAGMA foreign_keys=ON;
ALTER TABLE template RENAME TO templates;"###),(2,r###"PRAGMA foreign_keys=ON;
ALTER TABLE list ADD COLUMN topics JSON NOT NULL CHECK (json_type(topics) == 'array') DEFAULT '[]';"###,r###"PRAGMA foreign_keys=ON;
ALTER TABLE list DROP COLUMN topics;"###),(3,r###"PRAGMA foreign_keys=ON;

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
END;"###,r###"PRAGMA foreign_keys=ON;

DROP TRIGGER sort_topics_update_trigger;
DROP TRIGGER sort_topics_new_trigger;"###),(4,r###"CREATE TABLE IF NOT EXISTS settings_json_schema (
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
END;"###,r###"DROP TABLE settings_json_schema;
DROP TABLE list_settings_json;"###),(5,r###"INSERT OR REPLACE INTO settings_json_schema(id, value) VALUES('ArchivedAtLinkSettings', '{
  "$schema": "http://json-schema.org/draft-07/schema",
  "$ref": "#/$defs/ArchivedAtLinkSettings",
  "$defs": {
    "ArchivedAtLinkSettings": {
      "title": "ArchivedAtLinkSettings",
      "description": "Settings for ArchivedAtLink message filter",
      "type": "object",
      "properties": {
        "template": {
          "title": "Jinja template for header value",
          "description": "Template for\n        `Archived-At` header value, as described in RFC 5064 \"The Archived-At\n        Message Header Field\". The template receives only one string variable\n        with the value of the mailing list post `Message-ID` header.\n\n        For example, if:\n\n        - the template is `http://www.example.com/mid/{{msg_id}}`\n        - the `Message-ID` is `<0C2U00F01DFGCR@mailsj-v3.example.com>`\n\n        The full header will be generated as:\n\n        `Archived-At: <http://www.example.com/mid/0C2U00F01DFGCR@mailsj-v3.example.com>\n\n        Note: Surrounding carets in the `Message-ID` value are not required. If\n        you wish to preserve them in the URL, set option `preserve-carets` to\n        true.\n        ",
          "examples": [
            "https://www.example.com/{{msg_id}}",
            "https://www.example.com/{{msg_id}}.html"
          ],
          "type": "string",
          "pattern": ".+[{][{]msg_id[}][}].*"
        },
        "preserve_carets": {
          "title": "Preserve carets of `Message-ID` in generated value",
          "type": "boolean",
          "default": false
        }
      },
      "required": [
        "template"
      ]
    }
  }
}');"###,r###"DELETE FROM settings_json_schema WHERE id = 'ArchivedAtLinkSettings';"###),(6,r###"INSERT OR REPLACE INTO settings_json_schema(id, value) VALUES('AddSubjectTagPrefixSettings', '{
  "$schema": "http://json-schema.org/draft-07/schema",
  "$ref": "#/$defs/AddSubjectTagPrefixSettings",
  "$defs": {
    "AddSubjectTagPrefixSettings": {
      "title": "AddSubjectTagPrefixSettings",
      "description": "Settings for AddSubjectTagPrefix message filter",
      "type": "object",
      "properties": {
        "enabled": {
          "title": "If true, the list subject prefix is added to post subjects.",
          "type": "boolean"
        }
      },
      "required": [
        "enabled"
      ]
    }
  }
}');"###,r###"DELETE FROM settings_json_schema WHERE id = 'AddSubjectTagPrefixSettings';"###),(7,r###"INSERT OR REPLACE INTO settings_json_schema(id, value) VALUES('MimeRejectSettings', '{
  "$schema": "http://json-schema.org/draft-07/schema",
  "$ref": "#/$defs/MimeRejectSettings",
  "$defs": {
    "MimeRejectSettings": {
      "title": "MimeRejectSettings",
      "description": "Settings for MimeReject message filter",
      "type": "object",
      "properties": {
        "enabled": {
          "title": "If true, list posts that contain mime types in the reject array are rejected.",
          "type": "boolean"
        },
        "reject": {
          "title": "Mime types to reject.",
          "type": "array",
          "minLength": 0,
          "items": { "$ref": "#/$defs/MimeType" }
        },
        "required": [
          "enabled"
        ]
      }
    },
    "MimeType": {
      "type": "string",
      "maxLength": 127,
      "minLength": 3,
      "uniqueItems": true,
      "pattern": "^[a-zA-Z!#$&-^_]+[/][a-zA-Z!#$&-^_]+$"
    }
  }
}');"###,r###"DELETE FROM settings_json_schema WHERE id = 'MimeRejectSettings';"###),]