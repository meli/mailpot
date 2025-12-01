PRAGMA foreign_keys=OFF;
BEGIN TRANSACTION;
CREATE TABLE list (
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
  verify                BOOLEAN CHECK (verify IN (0, 1)) NOT NULL DEFAULT 1,
  hidden                BOOLEAN CHECK (hidden IN (0, 1)) NOT NULL DEFAULT 0,
  enabled               BOOLEAN CHECK (enabled IN (0, 1)) NOT NULL DEFAULT 1
);
INSERT INTO list VALUES(1,'foobar chat','foo-chat','foo-chat@example.com',NULL,NULL,NULL,NULL,1684009373,1684009373,1,0,1);
CREATE TABLE owner (
  pk               INTEGER PRIMARY KEY NOT NULL,
  list             INTEGER NOT NULL,
  address          TEXT NOT NULL,
  name             TEXT,
  created          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified    INTEGER NOT NULL DEFAULT (unixepoch()),
  FOREIGN KEY (list) REFERENCES list(pk) ON DELETE CASCADE
);
INSERT INTO owner VALUES(1,1,'user@example.com',NULL,1684257240,1684257240);
CREATE TABLE post_policy (
  pk                   INTEGER PRIMARY KEY NOT NULL,
  list                 INTEGER NOT NULL UNIQUE,
  announce_only        BOOLEAN CHECK (announce_only IN (0, 1)) NOT NULL
                       DEFAULT 0,
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
INSERT INTO post_policy VALUES(1,1,0,1,0,0,0,1684009373,1684009373);
CREATE TABLE subscription_policy (
  pk                   INTEGER PRIMARY KEY NOT NULL,
  list                 INTEGER NOT NULL UNIQUE,
  send_confirmation    BOOLEAN CHECK (send_confirmation IN (0, 1)) NOT NULL
                       DEFAULT 1,
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
INSERT INTO subscription_policy VALUES(1,1,1,1,0,0,0,1684258310,1684258310);
CREATE TABLE subscription (
  pk                      INTEGER PRIMARY KEY NOT NULL,
  list                    INTEGER NOT NULL,
  address                 TEXT NOT NULL,
  name                    TEXT,
  account                 INTEGER,
  enabled                 BOOLEAN CHECK (enabled IN (0, 1)) NOT NULL
                          DEFAULT 1,
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
INSERT INTO subscription VALUES(1,1,'user@example.com','Name',1,1,0,0,0,1,0,1,1684009373,1684009373,1684074388);
CREATE TABLE account (
  pk               INTEGER PRIMARY KEY NOT NULL,
  name             TEXT,
  address          TEXT NOT NULL UNIQUE,
  public_key       TEXT,
  password         TEXT NOT NULL,
  enabled          BOOLEAN CHECK (enabled IN (0, 1)) NOT NULL DEFAULT 1,
  created          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified    INTEGER NOT NULL DEFAULT (unixepoch())
);
INSERT INTO account VALUES(1,NULL,'user@example.com',NULL,'hunter2',1,1684074388,1684074388);
CREATE TABLE candidate_subscription (
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
CREATE TABLE post (
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
INSERT INTO post VALUES(1,1,NULL,'user@example.com','<abcdefgh@sator.example.com>',X'46726f6d3a204e616d65203c75736572406578616d706c652e636f6d3e0d0a546f3a203c666f6f2d63686174406578616d706c652e636f6d3e0d0a5375626a6563743a205b666f6f2d636861745d2054686973206973206120706f73740d0a446174653a205468752c203239204f637420323032302031333a35383a3136202b303030300d0a4d6573736167652d49443a203c6162636465666768407361746f722e6578616d706c652e636f6d3e0d0a436f6e74656e742d4c616e67756167653a20656e2d55530d0a436f6e74656e742d547970653a20746578742f68746d6c0d0a436f6e74656e742d5472616e736665722d456e636f64696e673a206261736536340d0a4d494d452d56657273696f6e3a20312e300d0a53656e6465723a203c666f6f2d63686174406578616d706c652e636f6d3e0d0a4c6973742d49643a203c666f6f2d636861742e6578616d706c652e636f6d3e0d0a4c6973742d48656c703a203c6d61696c746f3a666f6f2d636861742b72657175657374406578616d706c652e636f6d3f7375626a6563743d68656c703e0d0a4c6973742d506f73743a203c6d61696c746f3a666f6f2d63686174406578616d706c652e636f6d3e0d0a0d0a0d0a5043464554304e5557564246506a786f64473173506a786f5a57466b506a7830615852735a54356d623238384c3352706447786c506a7776614756685a443438596d396b0d0a6554343864474669624755675932786863334d39496d5a766279492b5048526f5a57466b506a7830636a34386447512b5a6d3976504339305a4434384c33526f5a57466b0d0a506a7830596d396b655434386448492b5048526b506d5a76627a45384c33526b506a77766448492b50433930596d396b655434384c335268596d786c506a7776596d396b0d0a655434384c3268306257772b0d0a',NULL,1603979896,'Thu, 29 Oct 2020 13:58:16 +0000',1684009373);
CREATE TABLE template (
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
CREATE TABLE queue (
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
INSERT INTO queue VALUES(1,'out',1,'PostAction::Reject { reason: Only subscriptions can post to this list. }','Name <user@example.com>','foo-chat+request@example.com','Your post to foo-chat was rejected.','<um5y5.zicw44b768p@example.com>',X'446174653a205361742c203133204d617920323032332032333a32323a3533202b303330300d0a46726f6d3a20666f6f2d636861742b72657175657374406578616d706c652e636f6d0d0a546f3a204e616d65203c75736572406578616d706c652e636f6d3e0d0a43633a200d0a4263633a200d0a5375626a6563743a20596f757220706f737420746f20666f6f2d63686174207761732072656a65637465642e0d0a4c6973742d49643a203c666f6f2d636861742e6578616d706c652e636f6d3e0d0a4c6973742d48656c703a203c6d61696c746f3a666f6f2d636861742b72657175657374406578616d706c652e636f6d3f7375626a6563743d68656c703e0d0a4c6973742d506f73743a203c6d61696c746f3a666f6f2d63686174406578616d706c652e636f6d3e0d0a4d6573736167652d49443a203c756d3579352e7a69637734346237363870406578616d706c652e636f6d3e0d0a4d494d452d56657273696f6e3a20312e300d0a436f6e74656e742d547970653a20746578742f706c61696e3b20636861727365743d227574662d38220d0a436f6e74656e742d5472616e736665722d456e636f64696e673a20386269740d0a0d0a4f6e6c7920737562736372697074696f6e732063616e20706f737420746f2074686973206c6973742e0d0a',1684009373,'2023-05-13 20:22:53.475684624+00:00');
INSERT INTO queue VALUES(2,'out',1,'subscription-confirmation','Name <user@example.com>','foo-chat+request@example.com','[foo-chat] You have successfully subscribed to foobar chat.','<um5y5.bvj2e888ql76@example.com>',X'446174653a205361742c203133204d617920323032332032333a32323a3533202b303330300d0a46726f6d3a20666f6f2d636861742b72657175657374406578616d706c652e636f6d0d0a546f3a204e616d65203c75736572406578616d706c652e636f6d3e0d0a43633a200d0a4263633a200d0a5375626a6563743a205b666f6f2d636861745d20596f752068617665207375636365737366756c6c79207375627363726962656420746f20666f6f62617220636861742e0d0a4c6973742d49643a203c666f6f2d636861742e6578616d706c652e636f6d3e0d0a4c6973742d48656c703a203c6d61696c746f3a666f6f2d636861742b72657175657374406578616d706c652e636f6d3f7375626a6563743d68656c703e0d0a4c6973742d506f73743a203c6d61696c746f3a666f6f2d63686174406578616d706c652e636f6d3e0d0a4d6573736167652d49443a203c756d3579352e62766a3265383838716c3736406578616d706c652e636f6d3e0d0a4d494d452d56657273696f6e3a20312e300d0a436f6e74656e742d547970653a20746578742f706c61696e3b20636861727365743d227574662d38220d0a436f6e74656e742d5472616e736665722d456e636f64696e673a20386269740d0a0d0a',1684009373,'2023-05-13 20:22:53.477293399+00:00');
CREATE TABLE bounce (
  pk              INTEGER PRIMARY KEY NOT NULL,
  subscription    INTEGER NOT NULL UNIQUE,
  count           INTEGER NOT NULL DEFAULT 0,
  last_bounce     TEXT NOT NULL DEFAULT (datetime()),
  FOREIGN KEY (subscription) REFERENCES subscription(pk) ON DELETE CASCADE
);
ANALYZE sqlite_schema;
INSERT INTO sqlite_stat1 VALUES('subscription','subscription_idx','1 1');
INSERT INTO sqlite_stat1 VALUES('subscription','sqlite_autoindex_subscription_1','1 1 1');
INSERT INTO sqlite_stat1 VALUES('post','post_msgid_idx','1 1');
INSERT INTO sqlite_stat1 VALUES('post','post_listpk_idx','1 1');
ANALYZE sqlite_schema;
CREATE TRIGGER accept_candidate AFTER INSERT ON subscription
FOR EACH ROW
BEGIN
  UPDATE candidate_subscription SET accepted = NEW.pk, last_modified = unixepoch()
  WHERE candidate_subscription.list = NEW.list AND candidate_subscription.address = NEW.address;
END;
CREATE TRIGGER verify_subscription_email AFTER INSERT ON subscription
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
CREATE TRIGGER add_account AFTER INSERT ON account
FOR EACH ROW
BEGIN
  UPDATE subscription SET account = NEW.pk, last_modified = unixepoch()
  WHERE subscription.address = NEW.address;
END;
CREATE TRIGGER add_account_to_subscription
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
CREATE TRIGGER last_modified_list
AFTER UPDATE ON list
FOR EACH ROW
WHEN NEW.last_modified != OLD.last_modified
BEGIN
  UPDATE list SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;
CREATE TRIGGER last_modified_owner
AFTER UPDATE ON owner
FOR EACH ROW
WHEN NEW.last_modified != OLD.last_modified
BEGIN
  UPDATE owner SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;
CREATE TRIGGER last_modified_post_policy
AFTER UPDATE ON post_policy
FOR EACH ROW
WHEN NEW.last_modified != OLD.last_modified
BEGIN
  UPDATE post_policy SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;
CREATE TRIGGER last_modified_subscription_policy
AFTER UPDATE ON subscription_policy
FOR EACH ROW
WHEN NEW.last_modified != OLD.last_modified
BEGIN
  UPDATE subscription_policy SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;
CREATE TRIGGER last_modified_subscription
AFTER UPDATE ON subscription
FOR EACH ROW
WHEN NEW.last_modified != OLD.last_modified
BEGIN
  UPDATE subscription SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;
CREATE TRIGGER last_modified_account
AFTER UPDATE ON account
FOR EACH ROW
WHEN NEW.last_modified != OLD.last_modified
BEGIN
  UPDATE account SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;
CREATE TRIGGER last_modified_candidate_subscription
AFTER UPDATE ON candidate_subscription
FOR EACH ROW
WHEN NEW.last_modified != OLD.last_modified
BEGIN
  UPDATE candidate_subscription SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;
CREATE TRIGGER last_modified_template
AFTER UPDATE ON template
FOR EACH ROW
WHEN NEW.last_modified != OLD.last_modified
BEGIN
  UPDATE template SET last_modified = unixepoch()
  WHERE pk = NEW.pk;
END;
CREATE INDEX post_listpk_idx ON post(list);
CREATE INDEX post_msgid_idx ON post(message_id);
CREATE INDEX list_idx ON list(id);
CREATE INDEX subscription_idx ON subscription(address);
COMMIT;
PRAGMA foreign_keys = true;
PRAGMA encoding = 'UTF-8';
PRAGMA user_version = 1;
