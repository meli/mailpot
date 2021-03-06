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
  announce_only BOOLEAN CHECK (announce_only in (0, 1)) NOT NULL DEFAULT 0,
  subscriber_only BOOLEAN CHECK (subscriber_only in (0, 1)) NOT NULL DEFAULT 0,
  approval_needed BOOLEAN CHECK (approval_needed in (0, 1)) NOT NULL DEFAULT 0,
  CHECK(((approval_needed) OR (((announce_only) OR (subscriber_only)) AND NOT ((announce_only) AND (subscriber_only)))) AND NOT ((approval_needed) AND (((announce_only) OR (subscriber_only)) AND NOT ((announce_only) AND (subscriber_only))))),
  FOREIGN KEY (list) REFERENCES mailing_lists(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS membership (
  list                    INTEGER NOT NULL,
  address                 TEXT NOT NULL,
  name                    TEXT,
  digest BOOLEAN CHECK (digest in (0, 1)) NOT NULL DEFAULT 0,
  hide_address BOOLEAN CHECK (hide_address in (0, 1)) NOT NULL DEFAULT 0,
  receive_duplicates BOOLEAN CHECK (receive_duplicates in (0, 1)) NOT NULL DEFAULT 1,
  receive_own_posts BOOLEAN CHECK (receive_own_posts in (0, 1)) NOT NULL DEFAULT 0,
  receive_confirmation BOOLEAN CHECK (receive_confirmation in (0, 1)) NOT NULL DEFAULT 1,
  PRIMARY KEY (list, address),
  FOREIGN KEY (list) REFERENCES mailing_lists(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS post (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  list                    INTEGER NOT NULL,
  address                 TEXT NOT NULL,
  message_id              TEXT NOT NULL,
  message                 BLOB NOT NULL,
  FOREIGN KEY (list, address) REFERENCES membership(list, address) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS post_event (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  post                    INTEGER NOT NULL,
  date                    INTEGER NOT NULL,
  kind                    CHAR(1) CHECK (kind IN ('R', 'S', 'D', 'B', 'O')) NOT NULL,
  content                 TEXT NOT NULL,
  FOREIGN KEY (post) REFERENCES post(pk) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS mailing_lists_idx ON mailing_lists(id);
CREATE INDEX IF NOT EXISTS membership_idx ON membership(address);
