define(xor, `dnl
(
    ($1) OR ($2)
  )
  AND NOT
  (
    ($1) AND ($2)
  )')dnl
dnl
dnl # Define boolean column types and defaults
define(BOOLEAN_TYPE, `BOOLEAN CHECK ($1 IN (0, 1)) NOT NULL')dnl
define(BOOLEAN_FALSE, `0')dnl
define(BOOLEAN_TRUE, `1')dnl
dnl
dnl # defile comment functions
dnl
dnl # Write the string '['+'tag'+':'+... with a macro so that tagref check
dnl # doesn't pick up on it as a duplicate.
define(__TAG, `tag')dnl
define(TAG, `['__TAG()`:$1]')dnl
dnl
dnl # define triggers
define(update_last_modified, `
-- 'TAG(last_modified_$1)`: update last_modified on every change.
CREATE TRIGGER
IF NOT EXISTS last_modified_$1
AFTER UPDATE ON $1
FOR EACH ROW
WHEN NEW.last_modified != OLD.last_modified
BEGIN
  UPDATE $1 SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;')dnl
dnl
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
  created               INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified         INTEGER NOT NULL DEFAULT (unixepoch()),
  verify                BOOLEAN_TYPE(verify) DEFAULT BOOLEAN_TRUE(),
  hidden                BOOLEAN_TYPE(hidden) DEFAULT BOOLEAN_FALSE(),
  enabled               BOOLEAN_TYPE(enabled) DEFAULT BOOLEAN_TRUE()
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
  announce_only        BOOLEAN_TYPE(announce_only)
                       DEFAULT BOOLEAN_FALSE(),
  subscription_only    BOOLEAN_TYPE(subscription_only)
                       DEFAULT BOOLEAN_FALSE(),
  approval_needed      BOOLEAN_TYPE(approval_needed)
                       DEFAULT BOOLEAN_FALSE(),
  open                 BOOLEAN_TYPE(open) DEFAULT BOOLEAN_FALSE(),
  custom               BOOLEAN_TYPE(custom) DEFAULT BOOLEAN_FALSE(),
  created              INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified        INTEGER NOT NULL DEFAULT (unixepoch())
  CHECK(xor(custom, xor(open, xor(approval_needed, xor(announce_only, subscription_only))))),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS subscription_policy (
  pk                   INTEGER PRIMARY KEY NOT NULL,
  list                 INTEGER NOT NULL UNIQUE,
  send_confirmation    BOOLEAN_TYPE(send_confirmation)
                       DEFAULT BOOLEAN_TRUE(),
  open                 BOOLEAN_TYPE(open) DEFAULT BOOLEAN_FALSE(),
  manual               BOOLEAN_TYPE(manual) DEFAULT BOOLEAN_FALSE(),
  request              BOOLEAN_TYPE(request) DEFAULT BOOLEAN_FALSE(),
  custom               BOOLEAN_TYPE(custom) DEFAULT BOOLEAN_FALSE(),
  created              INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified        INTEGER NOT NULL DEFAULT (unixepoch()),
  CHECK(xor(open, xor(manual, xor(request, custom)))),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS subscription (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  list                    INTEGER NOT NULL,
  address                 TEXT NOT NULL,
  name                    TEXT,
  account                 INTEGER,
  enabled                 BOOLEAN_TYPE(enabled)
                          DEFAULT BOOLEAN_TRUE(),
  verified                BOOLEAN_TYPE(verified)
                          DEFAULT BOOLEAN_TRUE(),
  digest                  BOOLEAN_TYPE(digest)
                          DEFAULT BOOLEAN_FALSE(),
  hide_address            BOOLEAN_TYPE(hide_address)
                          DEFAULT BOOLEAN_FALSE(),
  receive_duplicates      BOOLEAN_TYPE(receive_duplicates)
                          DEFAULT BOOLEAN_TRUE(),
  receive_own_posts       BOOLEAN_TYPE(receive_own_posts)
                          DEFAULT BOOLEAN_FALSE(),
  receive_confirmation    BOOLEAN_TYPE(receive_confirmation)
                          DEFAULT BOOLEAN_TRUE(),
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
  enabled          BOOLEAN_TYPE(enabled) DEFAULT BOOLEAN_TRUE(),
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

-- TAG(accept_candidate): Update candidacy with 'subscription' foreign key on
-- 'subscription' insert.
CREATE TRIGGER IF NOT EXISTS accept_candidate AFTER INSERT ON subscription
FOR EACH ROW
BEGIN
  UPDATE candidate_subscription SET accepted = NEW.pk, last_modified = unixepoch()
  WHERE candidate_subscription.list = NEW.list AND candidate_subscription.address = NEW.address;
END;

-- TAG(verify_subscription_email): If list settings require e-mail to be
-- verified, update new subscription's 'verify' column value.
CREATE TRIGGER IF NOT EXISTS verify_subscription_email AFTER INSERT ON subscription
FOR EACH ROW
BEGIN
  UPDATE subscription
  SET verified = BOOLEAN_FALSE(), last_modified = unixepoch()
  WHERE
  subscription.pk = NEW.pk
  AND
  EXISTS
  (SELECT 1 FROM list WHERE pk = NEW.list AND verify = BOOLEAN_TRUE());
END;

-- TAG(add_account): Update list subscription entries with 'account' foreign
-- key, if addresses match.
CREATE TRIGGER IF NOT EXISTS add_account AFTER INSERT ON account
FOR EACH ROW
BEGIN
  UPDATE subscription SET account = NEW.pk, last_modified = unixepoch()
  WHERE subscription.address = NEW.address;
END;

-- TAG(add_account_to_subscription): When adding a new 'subscription', auto
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

update_last_modified(`list')
update_last_modified(`owner')
update_last_modified(`post_policy')
update_last_modified(`subscription_policy')
update_last_modified(`subscription')
update_last_modified(`account')
update_last_modified(`candidate_subscription')
update_last_modified(`template')
