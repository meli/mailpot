PRAGMA foreign_keys = true;
PRAGMA encoding = 'UTF-8';

CREATE TABLE IF NOT EXISTS list (
  pk                       INTEGER PRIMARY KEY NOT NULL,
  name                     TEXT NOT NULL,
  id                       TEXT NOT NULL UNIQUE,
  address                  TEXT NOT NULL UNIQUE,
  owner_local_part         TEXT,
  request_local_part       TEXT,
  archive_url              TEXT,
  description              TEXT,
  created                  INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified            INTEGER NOT NULL DEFAULT (unixepoch()),
  verify BOOLEAN CHECK (verify in (0, 1)) NOT NULL     DEFAULT 1,
  hidden BOOLEAN CHECK (hidden in (0, 1)) NOT NULL     DEFAULT 0,
  enabled BOOLEAN CHECK (enabled in (0, 1)) NOT NULL    DEFAULT 1
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
  pk                               INTEGER PRIMARY KEY NOT NULL,
  list                             INTEGER NOT NULL UNIQUE,
  announce_only BOOLEAN CHECK (announce_only in (0, 1)) NOT NULL      DEFAULT 0,
  subscription_only BOOLEAN CHECK (subscription_only in (0, 1)) NOT NULL    DEFAULT 0,
  approval_needed BOOLEAN CHECK (approval_needed in (0, 1)) NOT NULL    DEFAULT 0,
  open BOOLEAN CHECK (open in (0, 1)) NOT NULL               DEFAULT 0,
  custom BOOLEAN CHECK (custom in (0, 1)) NOT NULL             DEFAULT 0,
  created                          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified                    INTEGER NOT NULL DEFAULT (unixepoch())
  CHECK(((custom) OR (((open) OR (((approval_needed) OR (((announce_only) OR (subscription_only)) AND NOT ((announce_only) AND (subscription_only)))) AND NOT ((approval_needed) AND (((announce_only) OR (subscription_only)) AND NOT ((announce_only) AND (subscription_only)))))) AND NOT ((open) AND (((approval_needed) OR (((announce_only) OR (subscription_only)) AND NOT ((announce_only) AND (subscription_only)))) AND NOT ((approval_needed) AND (((announce_only) OR (subscription_only)) AND NOT ((announce_only) AND (subscription_only)))))))) AND NOT ((custom) AND (((open) OR (((approval_needed) OR (((announce_only) OR (subscription_only)) AND NOT ((announce_only) AND (subscription_only)))) AND NOT ((approval_needed) AND (((announce_only) OR (subscription_only)) AND NOT ((announce_only) AND (subscription_only)))))) AND NOT ((open) AND (((approval_needed) OR (((announce_only) OR (subscription_only)) AND NOT ((announce_only) AND (subscription_only)))) AND NOT ((approval_needed) AND (((announce_only) OR (subscription_only)) AND NOT ((announce_only) AND (subscription_only))))))))),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS subscription_policy (
  pk                                 INTEGER PRIMARY KEY NOT NULL,
  list                               INTEGER NOT NULL UNIQUE,
  send_confirmation BOOLEAN CHECK (send_confirmation in (0, 1)) NOT NULL    DEFAULT 1,
  open BOOLEAN CHECK (open in (0, 1)) NOT NULL                 DEFAULT 0,
  manual BOOLEAN CHECK (manual in (0, 1)) NOT NULL               DEFAULT 0,
  request BOOLEAN CHECK (request in (0, 1)) NOT NULL              DEFAULT 0,
  custom BOOLEAN CHECK (custom in (0, 1)) NOT NULL               DEFAULT 0,
  created                            INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified                      INTEGER NOT NULL DEFAULT (unixepoch()),
  CHECK(((open) OR (((manual) OR (((request) OR (custom)) AND NOT ((request) AND (custom)))) AND NOT ((manual) AND (((request) OR (custom)) AND NOT ((request) AND (custom)))))) AND NOT ((open) AND (((manual) OR (((request) OR (custom)) AND NOT ((request) AND (custom)))) AND NOT ((manual) AND (((request) OR (custom)) AND NOT ((request) AND (custom))))))),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS subscription (
  pk                                    INTEGER PRIMARY KEY NOT NULL,
  list                                  INTEGER NOT NULL,
  address                               TEXT NOT NULL,
  name                                  TEXT,
  account                               INTEGER,
  enabled BOOLEAN CHECK (enabled in (0, 1)) NOT NULL                 DEFAULT 1,
  verified BOOLEAN CHECK (verified in (0, 1)) NOT NULL                DEFAULT 1,
  digest BOOLEAN CHECK (digest in (0, 1)) NOT NULL                  DEFAULT 0,
  hide_address BOOLEAN CHECK (hide_address in (0, 1)) NOT NULL            DEFAULT 0,
  receive_duplicates BOOLEAN CHECK (receive_duplicates in (0, 1)) NOT NULL      DEFAULT 1,
  receive_own_posts BOOLEAN CHECK (receive_own_posts in (0, 1)) NOT NULL       DEFAULT 0,
  receive_confirmation BOOLEAN CHECK (receive_confirmation in (0, 1)) NOT NULL    DEFAULT 1,
  created                               INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified                         INTEGER NOT NULL DEFAULT (unixepoch()),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE,
  FOREIGN KEY (account) REFERENCES account(pk) ON DELETE SET NULL,
  UNIQUE (list, address) ON CONFLICT ROLLBACK
);

CREATE TABLE IF NOT EXISTS account (
  pk                       INTEGER PRIMARY KEY NOT NULL,
  name                     TEXT,
  address                  TEXT NOT NULL UNIQUE,
  public_key               TEXT,
  password                 TEXT NOT NULL,
  enabled BOOLEAN CHECK (enabled in (0, 1)) NOT NULL    DEFAULT 1,
  created                  INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified            INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS candidate_subscription (
  pk                INTEGER PRIMARY KEY NOT NULL,
  list              INTEGER NOT NULL,
  address           TEXT NOT NULL,
  name              TEXT,
  accepted          INTEGER UNIQUE,
  created           INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified     INTEGER NOT NULL DEFAULT (unixepoch()),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE,
  FOREIGN KEY (accepted) REFERENCES subscription(pk) ON DELETE CASCADE,
  UNIQUE (list, address) ON CONFLICT ROLLBACK
);

CREATE TABLE IF NOT EXISTS post (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  list                    INTEGER NOT NULL,
  envelope_from           TEXT,
  address                 TEXT NOT NULL,
  message_id              TEXT NOT NULL,
  message                 BLOB NOT NULL,
  headers_json            TEXT,
  timestamp               INTEGER NOT NULL DEFAULT (unixepoch()),
  datetime                TEXT NOT NULL DEFAULT (datetime()),
  created                 INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS templates (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  name                    TEXT NOT NULL,
  list                    INTEGER,
  subject                 TEXT,
  headers_json            TEXT,
  body                    TEXT NOT NULL,
  created                 INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified           INTEGER NOT NULL DEFAULT (unixepoch()),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE,
  UNIQUE (list, name) ON CONFLICT ROLLBACK
);

-- # Queues
--
-- ## The "maildrop" queue
--
-- Messages that have been submitted but not yet processed, await processing in
-- the "maildrop" queue. Messages can be added to the "maildrop" queue even when
-- mailpot is not running.
--
-- ## The "deferred" queue
--
-- When all the deliverable recipients for a message are delivered, and for some
-- recipients delivery failed for a transient reason (it might succeed later), the
-- message is placed in the "deferred" queue.
--
-- ## The "hold" queue
--
-- List administrators may introduce rules for emails to be placed indefinitely in
-- the "hold" queue. Messages placed in the "hold" queue stay there until the
-- administrator intervenes. No periodic delivery attempts are made for messages
-- in the "hold" queue.

-- ## The "out" queue
--
-- Emails that must be sent as soon as possible.
CREATE TABLE IF NOT EXISTS queue (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  which                   TEXT CHECK (which IN ('maildrop', 'hold', 'deferred', 'corrupt', 'error', 'out')) NOT NULL,
  list                    INTEGER,
  comment                 TEXT,
  to_addresses            TEXT NOT NULL,
  from_address            TEXT NOT NULL,
  subject                 TEXT NOT NULL,
  message_id              TEXT NOT NULL UNIQUE,
  message                 BLOB NOT NULL,
  timestamp               INTEGER NOT NULL DEFAULT (unixepoch()),
  datetime                TEXT NOT NULL DEFAULT (datetime()),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS bounce (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  subscription            INTEGER NOT NULL UNIQUE,
  count                   INTEGER NOT NULL DEFAULT 0,
  last_bounce             TEXT NOT NULL DEFAULT (datetime()),
  FOREIGN KEY (subscription) REFERENCES subscription(pk) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS post_listpk_idx ON post(list);
CREATE INDEX IF NOT EXISTS post_msgid_idx ON post(message_id);
CREATE INDEX IF NOT EXISTS list_idx ON list(id);
CREATE INDEX IF NOT EXISTS subscription_idx ON subscription(address);

CREATE TRIGGER IF NOT EXISTS accept_candidate AFTER INSERT ON subscription
FOR EACH ROW
BEGIN
  UPDATE candidate_subscription SET accepted = NEW.pk, last_modified = unixepoch()
  WHERE candidate_subscription.list = NEW.list AND candidate_subscription.address = NEW.address;
END;

CREATE TRIGGER IF NOT EXISTS verify_candidate AFTER INSERT ON subscription
FOR EACH ROW
BEGIN
  UPDATE subscription SET verified = 0, last_modified = unixepoch()
  WHERE subscription.pk = NEW.pk AND EXISTS (SELECT 1 FROM list WHERE pk = NEW.list AND verify = 1);
END;

CREATE TRIGGER IF NOT EXISTS add_account AFTER INSERT ON account
FOR EACH ROW
BEGIN
  UPDATE subscription SET account = NEW.pk, last_modified = unixepoch()
  WHERE subscription.address = NEW.address;
END;

CREATE TRIGGER IF NOT EXISTS add_account_to_subscription AFTER INSERT ON subscription
FOR EACH ROW
BEGIN
  UPDATE subscription
     SET account = acc.pk,
         last_modified = unixepoch()
    FROM (SELECT * FROM account) AS acc
    WHERE subscription.account = acc.address;
END;

CREATE TRIGGER IF NOT EXISTS last_modified_list AFTER UPDATE ON list
FOR EACH ROW
BEGIN
  UPDATE list SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

CREATE TRIGGER IF NOT EXISTS last_modified_owner AFTER UPDATE ON owner
FOR EACH ROW
BEGIN
  UPDATE owner SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

CREATE TRIGGER IF NOT EXISTS last_modified_post_policy AFTER UPDATE ON post_policy
FOR EACH ROW
BEGIN
  UPDATE post_policy SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

CREATE TRIGGER IF NOT EXISTS last_modified_subscription_policy AFTER UPDATE ON subscription_policy
FOR EACH ROW
BEGIN
  UPDATE subscription_policy SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

CREATE TRIGGER IF NOT EXISTS last_modified_subscription AFTER UPDATE ON subscription
FOR EACH ROW
BEGIN
  UPDATE subscription SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

CREATE TRIGGER IF NOT EXISTS last_modified_account AFTER UPDATE ON account
FOR EACH ROW
BEGIN
  UPDATE account SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

CREATE TRIGGER IF NOT EXISTS last_modified_candidate_subscription AFTER UPDATE ON candidate_subscription
FOR EACH ROW
BEGIN
  UPDATE candidate_subscription SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;

CREATE TRIGGER IF NOT EXISTS last_modified_templates AFTER UPDATE ON templates
FOR EACH ROW
BEGIN
  UPDATE templates SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;
