define(xor, `(($1) OR ($2)) AND NOT (($1) AND ($2))')dnl
define(BOOLEAN_TYPE, `$1 BOOLEAN CHECK ($1 in (0, 1)) NOT NULL')dnl
define(BOOLEAN_FALSE, `0')dnl
define(BOOLEAN_TRUE, `1')dnl
define(update_last_modified, `CREATE TRIGGER IF NOT EXISTS last_modified_$1 AFTER UPDATE ON $1
FOR EACH ROW
BEGIN
  UPDATE $1 SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;')dnl
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
  BOOLEAN_TYPE(verify)     DEFAULT BOOLEAN_TRUE(),
  BOOLEAN_TYPE(hidden)     DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(enabled)    DEFAULT BOOLEAN_TRUE()
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
  BOOLEAN_TYPE(announce_only)      DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(subscription_only)    DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(approval_needed)    DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(open)               DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(custom)             DEFAULT BOOLEAN_FALSE(),
  created                          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified                    INTEGER NOT NULL DEFAULT (unixepoch())
  CHECK(xor(custom, xor(open, xor(approval_needed, xor(announce_only, subscription_only))))),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS subscription_policy (
  pk                                 INTEGER PRIMARY KEY NOT NULL,
  list                               INTEGER NOT NULL UNIQUE,
  BOOLEAN_TYPE(send_confirmation)    DEFAULT BOOLEAN_TRUE(),
  BOOLEAN_TYPE(open)                 DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(manual)               DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(request)              DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(custom)               DEFAULT BOOLEAN_FALSE(),
  created                            INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified                      INTEGER NOT NULL DEFAULT (unixepoch()),
  CHECK(xor(open, xor(manual, xor(request, custom)))),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS subscription (
  pk                                    INTEGER PRIMARY KEY NOT NULL,
  list                                  INTEGER NOT NULL,
  address                               TEXT NOT NULL,
  name                                  TEXT,
  account                               INTEGER,
  BOOLEAN_TYPE(enabled)                 DEFAULT BOOLEAN_TRUE(),
  BOOLEAN_TYPE(verified)                DEFAULT BOOLEAN_TRUE(),
  BOOLEAN_TYPE(digest)                  DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(hide_address)            DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(receive_duplicates)      DEFAULT BOOLEAN_TRUE(),
  BOOLEAN_TYPE(receive_own_posts)       DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(receive_confirmation)    DEFAULT BOOLEAN_TRUE(),
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
  BOOLEAN_TYPE(enabled)    DEFAULT BOOLEAN_TRUE(),
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
  list                    INTEGER UNIQUE,
  subject                 TEXT,
  body                    TEXT NOT NULL,
  created                 INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified           INTEGER NOT NULL DEFAULT (unixepoch())
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
  UPDATE subscription SET verified = BOOLEAN_FALSE(), last_modified = unixepoch()
  WHERE subscription.pk = NEW.pk AND EXISTS (SELECT 1 FROM list WHERE pk = NEW.list AND verify = BOOLEAN_TRUE());
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

update_last_modified(`list')

update_last_modified(`owner')

update_last_modified(`post_policy')

update_last_modified(`subscription_policy')

update_last_modified(`subscription')

update_last_modified(`account')

update_last_modified(`candidate_subscription')

update_last_modified(`templates')
