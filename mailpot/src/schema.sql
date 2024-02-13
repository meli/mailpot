PRAGMA foreign_keys = true;
PRAGMA encoding = 'UTF-8';

CREATE TABLE IF NOT EXISTS list (
  pk                    INTEGER PRIMARY KEY NOT NULL,
  name                  TEXT NOT NULL,
  id                    TEXT NOT NULL UNIQUE,
  address               TEXT NOT NULL UNIQUE,
  owner_local_part      TEXT,
  request_local_part    TEXT,
  archive_url           TEXT,
  description           TEXT,
  topics                JSON NOT NULL CHECK (json_type(topics) = 'array') DEFAULT '[]',
  created               INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified         INTEGER NOT NULL DEFAULT (unixepoch()),
  verify                BOOLEAN CHECK (verify IN (0, 1)) NOT NULL DEFAULT 1, -- BOOLEAN FALSE == 0, BOOLEAN TRUE == 1
  hidden                BOOLEAN CHECK (hidden IN (0, 1)) NOT NULL DEFAULT 0,
  enabled               BOOLEAN CHECK (enabled IN (0, 1)) NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS owner (
  pk               INTEGER PRIMARY KEY NOT NULL,
  list             INTEGER NOT NULL,
  address          TEXT NOT NULL,
  name             TEXT,
  created          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified    INTEGER NOT NULL DEFAULT (unixepoch()),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS post_policy (
  pk                   INTEGER PRIMARY KEY NOT NULL,
  list                 INTEGER NOT NULL UNIQUE,
  announce_only        BOOLEAN CHECK (announce_only IN (0, 1)) NOT NULL
                       DEFAULT 0, -- BOOLEAN FALSE == 0, BOOLEAN TRUE == 1
  subscription_only    BOOLEAN CHECK (subscription_only IN (0, 1)) NOT NULL
                       DEFAULT 0,
  approval_needed      BOOLEAN CHECK (approval_needed IN (0, 1)) NOT NULL
                       DEFAULT 0,
  open                 BOOLEAN CHECK (open IN (0, 1)) NOT NULL DEFAULT 0,
  custom               BOOLEAN CHECK (custom IN (0, 1)) NOT NULL DEFAULT 0,
  created              INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified        INTEGER NOT NULL DEFAULT (unixepoch())
  CHECK((
    (custom) OR ((
    (open) OR ((
    (approval_needed) OR ((
    (announce_only) OR (subscription_only)
  )
  AND NOT
  (
    (announce_only) AND (subscription_only)
  ))
  )
  AND NOT
  (
    (approval_needed) AND ((
    (announce_only) OR (subscription_only)
  )
  AND NOT
  (
    (announce_only) AND (subscription_only)
  ))
  ))
  )
  AND NOT
  (
    (open) AND ((
    (approval_needed) OR ((
    (announce_only) OR (subscription_only)
  )
  AND NOT
  (
    (announce_only) AND (subscription_only)
  ))
  )
  AND NOT
  (
    (approval_needed) AND ((
    (announce_only) OR (subscription_only)
  )
  AND NOT
  (
    (announce_only) AND (subscription_only)
  ))
  ))
  ))
  )
  AND NOT
  (
    (custom) AND ((
    (open) OR ((
    (approval_needed) OR ((
    (announce_only) OR (subscription_only)
  )
  AND NOT
  (
    (announce_only) AND (subscription_only)
  ))
  )
  AND NOT
  (
    (approval_needed) AND ((
    (announce_only) OR (subscription_only)
  )
  AND NOT
  (
    (announce_only) AND (subscription_only)
  ))
  ))
  )
  AND NOT
  (
    (open) AND ((
    (approval_needed) OR ((
    (announce_only) OR (subscription_only)
  )
  AND NOT
  (
    (announce_only) AND (subscription_only)
  ))
  )
  AND NOT
  (
    (approval_needed) AND ((
    (announce_only) OR (subscription_only)
  )
  AND NOT
  (
    (announce_only) AND (subscription_only)
  ))
  ))
  ))
  )),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS subscription_policy (
  pk                   INTEGER PRIMARY KEY NOT NULL,
  list                 INTEGER NOT NULL UNIQUE,
  send_confirmation    BOOLEAN CHECK (send_confirmation IN (0, 1)) NOT NULL
                       DEFAULT 1, -- BOOLEAN FALSE == 0, BOOLEAN TRUE == 1
  open                 BOOLEAN CHECK (open IN (0, 1)) NOT NULL DEFAULT 0,
  manual               BOOLEAN CHECK (manual IN (0, 1)) NOT NULL DEFAULT 0,
  request              BOOLEAN CHECK (request IN (0, 1)) NOT NULL DEFAULT 0,
  custom               BOOLEAN CHECK (custom IN (0, 1)) NOT NULL DEFAULT 0,
  created              INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified        INTEGER NOT NULL DEFAULT (unixepoch()),
  CHECK((
    (open) OR ((
    (manual) OR ((
    (request) OR (custom)
  )
  AND NOT
  (
    (request) AND (custom)
  ))
  )
  AND NOT
  (
    (manual) AND ((
    (request) OR (custom)
  )
  AND NOT
  (
    (request) AND (custom)
  ))
  ))
  )
  AND NOT
  (
    (open) AND ((
    (manual) OR ((
    (request) OR (custom)
  )
  AND NOT
  (
    (request) AND (custom)
  ))
  )
  AND NOT
  (
    (manual) AND ((
    (request) OR (custom)
  )
  AND NOT
  (
    (request) AND (custom)
  ))
  ))
  )),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS subscription (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  list                    INTEGER NOT NULL,
  address                 TEXT NOT NULL,
  name                    TEXT,
  account                 INTEGER,
  enabled                 BOOLEAN CHECK (enabled IN (0, 1)) NOT NULL
                          DEFAULT 1, -- BOOLEAN FALSE == 0, BOOLEAN TRUE == 1
  verified                BOOLEAN CHECK (verified IN (0, 1)) NOT NULL
                          DEFAULT 1,
  digest                  BOOLEAN CHECK (digest IN (0, 1)) NOT NULL
                          DEFAULT 0,
  hide_address            BOOLEAN CHECK (hide_address IN (0, 1)) NOT NULL
                          DEFAULT 0,
  receive_duplicates      BOOLEAN CHECK (receive_duplicates IN (0, 1)) NOT NULL
                          DEFAULT 1,
  receive_own_posts       BOOLEAN CHECK (receive_own_posts IN (0, 1)) NOT NULL
                          DEFAULT 0,
  receive_confirmation    BOOLEAN CHECK (receive_confirmation IN (0, 1)) NOT NULL
                          DEFAULT 1,
  last_digest             INTEGER NOT NULL DEFAULT (unixepoch()),
  created                 INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified           INTEGER NOT NULL DEFAULT (unixepoch()),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE,
  FOREIGN KEY (account) REFERENCES account(pk) ON DELETE SET NULL,
  UNIQUE (list, address) ON CONFLICT ROLLBACK
);

CREATE TABLE IF NOT EXISTS account (
  pk               INTEGER PRIMARY KEY NOT NULL,
  name             TEXT,
  address          TEXT NOT NULL UNIQUE,
  public_key       TEXT,
  password         TEXT NOT NULL,
  enabled          BOOLEAN CHECK (enabled IN (0, 1)) NOT NULL DEFAULT 1, -- BOOLEAN FALSE == 0, BOOLEAN TRUE == 1
  created          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified    INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS candidate_subscription (
  pk               INTEGER PRIMARY KEY NOT NULL,
  list             INTEGER NOT NULL,
  address          TEXT NOT NULL,
  name             TEXT,
  accepted         INTEGER UNIQUE,
  created          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified    INTEGER NOT NULL DEFAULT (unixepoch()),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE,
  FOREIGN KEY (accepted) REFERENCES subscription(pk) ON DELETE CASCADE,
  UNIQUE (list, address) ON CONFLICT ROLLBACK
);

CREATE TABLE IF NOT EXISTS post (
  pk               INTEGER PRIMARY KEY NOT NULL,
  list             INTEGER NOT NULL,
  envelope_from    TEXT,
  address          TEXT NOT NULL,
  message_id       TEXT NOT NULL,
  message          BLOB NOT NULL,
  headers_json     TEXT,
  timestamp        INTEGER NOT NULL DEFAULT (unixepoch()),
  datetime         TEXT NOT NULL DEFAULT (datetime()),
  created          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS template (
  pk               INTEGER PRIMARY KEY NOT NULL,
  name             TEXT NOT NULL,
  list             INTEGER,
  subject          TEXT,
  headers_json     TEXT,
  body             TEXT NOT NULL,
  created          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified    INTEGER NOT NULL DEFAULT (unixepoch()),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE,
  UNIQUE (list, name) ON CONFLICT ROLLBACK
);

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
  is_valid         BOOLEAN CHECK (is_valid IN (0, 1)) NOT NULL DEFAULT 0, -- BOOLEAN FALSE == 0, BOOLEAN TRUE == 1
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

-- # Queues
--
-- ## The "maildrop" queue
--
-- Messages that have been submitted but not yet processed, await processing
-- in the "maildrop" queue. Messages can be added to the "maildrop" queue
-- even when mailpot is not running.
--
-- ## The "deferred" queue
--
-- When all the deliverable recipients for a message are delivered, and for
-- some recipients delivery failed for a transient reason (it might succeed
-- later), the message is placed in the "deferred" queue.
--
-- ## The "hold" queue
--
-- List administrators may introduce rules for emails to be placed
-- indefinitely in the "hold" queue. Messages placed in the "hold" queue stay
-- there until the administrator intervenes. No periodic delivery attempts
-- are made for messages in the "hold" queue.

-- ## The "out" queue
--
-- Emails that must be sent as soon as possible.
CREATE TABLE IF NOT EXISTS queue (
  pk              INTEGER PRIMARY KEY NOT NULL,
  which           TEXT
                  CHECK (
                    which IN
                    ('maildrop',
                     'hold',
                     'deferred',
                     'corrupt',
                     'error',
                     'out')
                  ) NOT NULL,
  list            INTEGER,
  comment         TEXT,
  to_addresses    TEXT NOT NULL,
  from_address    TEXT NOT NULL,
  subject         TEXT NOT NULL,
  message_id      TEXT NOT NULL,
  message         BLOB NOT NULL,
  timestamp       INTEGER NOT NULL DEFAULT (unixepoch()),
  datetime        TEXT NOT NULL DEFAULT (datetime()),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE,
  UNIQUE (to_addresses, message_id) ON CONFLICT ROLLBACK
);

CREATE TABLE IF NOT EXISTS bounce (
  pk              INTEGER PRIMARY KEY NOT NULL,
  subscription    INTEGER NOT NULL UNIQUE,
  count           INTEGER NOT NULL DEFAULT 0,
  last_bounce     TEXT NOT NULL DEFAULT (datetime()),
  FOREIGN KEY (subscription) REFERENCES subscription(pk) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS post_listpk_idx ON post(list);
CREATE INDEX IF NOT EXISTS post_msgid_idx ON post(message_id);
CREATE INDEX IF NOT EXISTS list_idx ON list(id);
CREATE INDEX IF NOT EXISTS subscription_idx ON subscription(address);

-- [tag:accept_candidate]: Update candidacy with 'subscription' foreign key on
-- 'subscription' insert.
CREATE TRIGGER IF NOT EXISTS accept_candidate AFTER INSERT ON subscription
FOR EACH ROW
BEGIN
  UPDATE candidate_subscription SET accepted = NEW.pk, last_modified = unixepoch()
  WHERE candidate_subscription.list = NEW.list AND candidate_subscription.address = NEW.address;
END;

-- [tag:verify_subscription_email]: If list settings require e-mail to be
-- verified, update new subscription's 'verify' column value.
CREATE TRIGGER IF NOT EXISTS verify_subscription_email AFTER INSERT ON subscription
FOR EACH ROW
BEGIN
  UPDATE subscription
  SET verified = 0, last_modified = unixepoch()
  WHERE
  subscription.pk = NEW.pk
  AND
  EXISTS
  (SELECT 1 FROM list WHERE pk = NEW.list AND verify = 1);
END;

-- [tag:add_account]: Update list subscription entries with 'account' foreign
-- key, if addresses match.
CREATE TRIGGER IF NOT EXISTS add_account AFTER INSERT ON account
FOR EACH ROW
BEGIN
  UPDATE subscription SET account = NEW.pk, last_modified = unixepoch()
  WHERE subscription.address = NEW.address;
END;

-- [tag:add_account_to_subscription]: When adding a new 'subscription', auto
-- set 'account' value if there already exists an 'account' entry with the
-- same address.
CREATE TRIGGER IF NOT EXISTS add_account_to_subscription
AFTER INSERT ON subscription
FOR EACH ROW
WHEN
  NEW.account IS NULL
  AND EXISTS (SELECT 1 FROM account WHERE address = NEW.address)
BEGIN
  UPDATE subscription
     SET account = (SELECT pk FROM account WHERE address = NEW.address),
         last_modified = unixepoch()
    WHERE subscription.pk = NEW.pk;
END;


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


-- 005.data.sql

INSERT OR REPLACE INTO settings_json_schema(id, value) VALUES('ArchivedAtLinkSettings', '{
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
}');


-- 006.data.sql

INSERT OR REPLACE INTO settings_json_schema(id, value) VALUES('AddSubjectTagPrefixSettings', '{
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
}');


-- 007.data.sql

INSERT OR REPLACE INTO settings_json_schema(id, value) VALUES('MimeRejectSettings', '{
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
}');


-- Set current schema version.

PRAGMA user_version = 7;
