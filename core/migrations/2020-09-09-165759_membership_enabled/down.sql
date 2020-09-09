-- This file should undo anything in `up.sql`

BEGIN TRANSACTION;
PRAGMA foreign_keys = false;
CREATE TEMPORARY TABLE membership_backup(
  list                    INTEGER NOT NULL,
  address                 TEXT NOT NULL,
  name                    TEXT,
  enabled BOOLEAN CHECK (enabled in (0, 1)) NOT NULL DEFAULT 1,
  digest BOOLEAN CHECK (digest in (0, 1)) NOT NULL DEFAULT 0,
  hide_address BOOLEAN CHECK (hide_address in (0, 1)) NOT NULL DEFAULT 0,
  receive_duplicates BOOLEAN CHECK (receive_duplicates in (0, 1)) NOT NULL DEFAULT 1,
  receive_own_posts BOOLEAN CHECK (receive_own_posts in (0, 1)) NOT NULL DEFAULT 0,
  receive_confirmation BOOLEAN CHECK (receive_confirmation in (0, 1)) NOT NULL DEFAULT 1,
  PRIMARY KEY (list, address),
  FOREIGN KEY (list) REFERENCES mailing_lists(pk) ON DELETE CASCADE
);
INSERT INTO membership_backup SELECT list,address,name,enabled,digest,hide_address,receive_own_posts,receive_duplicates,receive_confirmation FROM membership;
DROP TABLE membership;
CREATE TABLE membership(
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
INSERT INTO membership SELECT list,address,name,digest,hide_address,receive_own_posts,receive_duplicates,receive_confirmation FROM membership_backup;
DROP TABLE membership_backup;
PRAGMA foreign_keys = true;
COMMIT TRANSACTION;
