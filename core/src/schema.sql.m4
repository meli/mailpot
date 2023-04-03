define(xor, `(($1) OR ($2)) AND NOT (($1) AND ($2))')dnl
define(BOOLEAN_TYPE, `$1 BOOLEAN CHECK ($1 in (0, 1)) NOT NULL')dnl
define(BOOLEAN_FALSE, `0')dnl
define(BOOLEAN_TRUE, `1')dnl
PRAGMA foreign_keys = true;
PRAGMA encoding = 'UTF-8';

CREATE TABLE IF NOT EXISTS mailing_lists (
  pk              INTEGER PRIMARY KEY NOT NULL,
  name            TEXT NOT NULL,
  id              TEXT NOT NULL,
  address         TEXT NOT NULL,
  archive_url     TEXT,
  description     TEXT
);

CREATE TABLE IF NOT EXISTS list_owner (
  pk              INTEGER PRIMARY KEY NOT NULL,
  list            INTEGER NOT NULL,
  address         TEXT NOT NULL,
  name            TEXT,
  FOREIGN KEY (list) REFERENCES mailing_lists(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS post_policy (
  pk              INTEGER PRIMARY KEY NOT NULL,
  list            INTEGER NOT NULL UNIQUE,
  BOOLEAN_TYPE(announce_only) DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(subscriber_only) DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(approval_needed) DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(no_subscriptions) DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(custom) DEFAULT BOOLEAN_FALSE(),
  CHECK(xor(custom, xor(no_subscriptions, xor(approval_needed, xor(announce_only, subscriber_only))))),
  FOREIGN KEY (list) REFERENCES mailing_lists(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS membership (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  list                    INTEGER NOT NULL,
  address                 TEXT NOT NULL,
  name                    TEXT,
  account                 INTEGER,
  BOOLEAN_TYPE(enabled) DEFAULT BOOLEAN_TRUE(),
  BOOLEAN_TYPE(digest) DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(hide_address) DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(receive_duplicates) DEFAULT BOOLEAN_TRUE(),
  BOOLEAN_TYPE(receive_own_posts) DEFAULT BOOLEAN_FALSE(),
  BOOLEAN_TYPE(receive_confirmation) DEFAULT BOOLEAN_TRUE(),
  FOREIGN KEY (list) REFERENCES mailing_lists(pk) ON DELETE CASCADE,
  FOREIGN KEY (account) REFERENCES account(pk) ON DELETE CASCADE,
  UNIQUE (list, address) ON CONFLICT ROLLBACK
);

CREATE TABLE IF NOT EXISTS account (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  name                    TEXT,
  address                 TEXT NOT NULL UNIQUE,
  public_key              TEXT,
  password                TEXT NOT NULL,
  BOOLEAN_TYPE(enabled) DEFAULT BOOLEAN_TRUE()
);

CREATE TABLE IF NOT EXISTS candidate_membership (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  list                    INTEGER NOT NULL,
  address                 TEXT NOT NULL,
  name                    TEXT,
  accepted                INTEGER,
  FOREIGN KEY (list) REFERENCES mailing_lists(pk) ON DELETE CASCADE,
  FOREIGN KEY (accepted) REFERENCES membership(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS post (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  list                    INTEGER NOT NULL,
  address                 TEXT NOT NULL,
  message_id              TEXT NOT NULL,
  message                 BLOB NOT NULL,
  timestamp               INTEGER NOT NULL DEFAULT (unixepoch()),
  datetime                TEXT NOT NULL DEFAULT (datetime())
);

CREATE TABLE IF NOT EXISTS post_event (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  post                    INTEGER NOT NULL,
  date                    INTEGER NOT NULL,
  kind                    CHAR(1) CHECK (kind IN ('R', 'S', 'D', 'B', 'O')) NOT NULL,
  content                 TEXT NOT NULL,
  FOREIGN KEY (post) REFERENCES post(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS error_queue (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  error                   TEXT NOT NULL,
  to_address              TEXT NOT NULL,
  from_address            TEXT NOT NULL,
  subject                 TEXT NOT NULL,
  message_id              TEXT NOT NULL,
  message                 BLOB NOT NULL,
  timestamp               INTEGER NOT NULL DEFAULT (unixepoch()),
  datetime                TEXT NOT NULL DEFAULT (datetime())
);

-- # Queues
--
-- ## The "maildrop" queue
--
-- Messages that have been submitted  but not yet processed, await processing in
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
CREATE TABLE IF NOT EXISTS queue (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  kind                    TEXT CHECK (kind IN ('maildrop', 'hold', 'deferred', 'corrupt')) NOT NULL,
  to_addresses            TEXT NOT NULL,
  from_address            TEXT NOT NULL,
  subject                 TEXT NOT NULL,
  message_id              TEXT NOT NULL,
  message                 BLOB NOT NULL,
  timestamp               INTEGER NOT NULL DEFAULT (unixepoch()),
  datetime                TEXT NOT NULL DEFAULT (datetime())
);


CREATE INDEX IF NOT EXISTS post_listpk_idx ON post(list);
CREATE INDEX IF NOT EXISTS post_msgid_idx ON post(message_id);
CREATE INDEX IF NOT EXISTS mailing_lists_idx ON mailing_lists(id);
CREATE INDEX IF NOT EXISTS membership_idx ON membership(address);
