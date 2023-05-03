/*
 * This file is part of mailpot
 *
 * Copyright 2020 - Manos Pitsidianakis
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of the
 * License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */

use mailpot::{models::*, queue::Queue, Configuration, Connection, SendMail};
use mailpot_tests::init_stderr_logging;
use tempfile::TempDir;

#[test]
fn test_list_subscription() {
    init_stderr_logging();

    let tmp_dir = TempDir::new().unwrap();

    let db_path = tmp_dir.path().join("mpot.db");
    let config = Configuration {
        send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
        administrators: vec![],
    };

    let db = Connection::open_or_create_db(config).unwrap().trusted();
    assert!(db.lists().unwrap().is_empty());
    let foo_chat = db
        .create_list(MailingList {
            pk: 0,
            name: "foobar chat".into(),
            id: "foo-chat".into(),
            address: "foo-chat@example.com".into(),
            description: None,
            archive_url: None,
        })
        .unwrap();

    assert_eq!(foo_chat.pk(), 1);
    let lists = db.lists().unwrap();
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0], foo_chat);
    let post_policy = db
        .set_list_post_policy(PostPolicy {
            pk: 0,
            list: foo_chat.pk(),
            announce_only: false,
            subscription_only: true,
            approval_needed: false,
            open: false,
            custom: false,
        })
        .unwrap();

    assert_eq!(post_policy.pk(), 1);
    assert_eq!(db.queue(Queue::Error).unwrap().len(), 0);
    assert_eq!(db.list_subscriptions(foo_chat.pk()).unwrap().len(), 0);

    let mut db = db.untrusted();

    let post_bytes = b"From: Name <user@example.com>
To: <foo-chat@example.com>
Subject: This is a post
Date: Thu, 29 Oct 2020 13:58:16 +0000
Message-ID: <abcdefgh@sator.example.com>
Content-Language: en-US
Content-Type: text/html
Content-Transfer-Encoding: base64
MIME-Version: 1.0

PCFET0NUWVBFPjxodG1sPjxoZWFkPjx0aXRsZT5mb288L3RpdGxlPjwvaGVhZD48Ym9k
eT48dGFibGUgY2xhc3M9ImZvbyI+PHRoZWFkPjx0cj48dGQ+Zm9vPC90ZD48L3RoZWFk
Pjx0Ym9keT48dHI+PHRkPmZvbzE8L3RkPjwvdHI+PC90Ym9keT48L3RhYmxlPjwvYm9k
eT48L2h0bWw+
";
    let envelope = melib::Envelope::from_bytes(post_bytes, None).expect("Could not parse message");
    db.post(&envelope, post_bytes, /* dry_run */ false)
        .expect("Got unexpected error");
    let out = db.queue(Queue::Out).unwrap();
    assert_eq!(out.len(), 1);
    const COMMENT_PREFIX: &str = "PostAction::Reject { reason: Only subscriptions";
    assert_eq!(
        out[0]
            .comment
            .as_ref()
            .and_then(|c| c.get(..COMMENT_PREFIX.len())),
        Some(COMMENT_PREFIX)
    );

    let subscribe_bytes = b"From: Name <user@example.com>
To: <foo-chat+subscribe@example.com>
Subject: subscribe
Date: Thu, 29 Oct 2020 13:58:16 +0000
Message-ID: <abcdefgh@sator.example.com>
Content-Language: en-US
Content-Type: text/html
Content-Transfer-Encoding: base64
MIME-Version: 1.0

";
    let envelope =
        melib::Envelope::from_bytes(subscribe_bytes, None).expect("Could not parse message");
    db.post(&envelope, subscribe_bytes, /* dry_run */ false)
        .unwrap();
    assert_eq!(db.list_subscriptions(foo_chat.pk()).unwrap().len(), 1);
    assert_eq!(db.queue(Queue::Out).unwrap().len(), 2);
    let envelope = melib::Envelope::from_bytes(post_bytes, None).expect("Could not parse message");
    db.post(&envelope, post_bytes, /* dry_run */ false).unwrap();
    assert_eq!(db.queue(Queue::Out).unwrap().len(), 2);
    assert_eq!(db.list_posts(foo_chat.pk(), None).unwrap().len(), 1);
}

#[test]
fn test_post_rejection() {
    init_stderr_logging();

    const ANNOUNCE_ONLY_PREFIX: Option<&str> =
        Some("PostAction::Reject { reason: You are not allowed to post on this list.");
    const APPROVAL_ONLY_PREFIX: Option<&str> = Some(
        "PostAction::Defer { reason: Your posting has been deferred. Approval from the list's \
         moderators",
    );

    for (q, mut post_policy) in [
        (
            [(Queue::Out, ANNOUNCE_ONLY_PREFIX)].as_slice(),
            PostPolicy {
                pk: -1,
                list: -1,
                announce_only: true,
                subscription_only: false,
                approval_needed: false,
                open: false,
                custom: false,
            },
        ),
        (
            [(Queue::Out, APPROVAL_ONLY_PREFIX), (Queue::Deferred, None)].as_slice(),
            PostPolicy {
                pk: -1,
                list: -1,
                announce_only: false,
                subscription_only: false,
                approval_needed: true,
                open: false,
                custom: false,
            },
        ),
    ] {
        let tmp_dir = TempDir::new().unwrap();

        let db_path = tmp_dir.path().join("mpot.db");
        let config = Configuration {
            send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
            db_path,
            data_path: tmp_dir.path().to_path_buf(),
            administrators: vec![],
        };

        let db = Connection::open_or_create_db(config).unwrap().trusted();
        assert!(db.lists().unwrap().is_empty());
        let foo_chat = db
            .create_list(MailingList {
                pk: 0,
                name: "foobar chat".into(),
                id: "foo-chat".into(),
                address: "foo-chat@example.com".into(),
                description: None,
                archive_url: None,
            })
            .unwrap();

        assert_eq!(foo_chat.pk(), 1);
        let lists = db.lists().unwrap();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0], foo_chat);
        post_policy.list = foo_chat.pk();
        let post_policy = db.set_list_post_policy(post_policy).unwrap();

        assert_eq!(post_policy.pk(), 1);
        assert_eq!(db.queue(Queue::Error).unwrap().len(), 0);
        assert_eq!(db.list_subscriptions(foo_chat.pk()).unwrap().len(), 0);

        let mut db = db.untrusted();

        let post_bytes = b"From: Name <user@example.com>
To: <foo-chat@example.com>
Subject: This is a post
Date: Thu, 29 Oct 2020 13:58:16 +0000
Message-ID: <abcdefgh@sator.example.com>
Content-Language: en-US
Content-Type: text/html
Content-Transfer-Encoding: base64
MIME-Version: 1.0

PCFET0NUWVBFPjxodG1sPjxoZWFkPjx0aXRsZT5mb288L3RpdGxlPjwvaGVhZD48Ym9k
eT48dGFibGUgY2xhc3M9ImZvbyI+PHRoZWFkPjx0cj48dGQ+Zm9vPC90ZD48L3RoZWFk
Pjx0Ym9keT48dHI+PHRkPmZvbzE8L3RkPjwvdHI+PC90Ym9keT48L3RhYmxlPjwvYm9k
eT48L2h0bWw+
";
        let envelope =
            melib::Envelope::from_bytes(post_bytes, None).expect("Could not parse message");
        db.post(&envelope, post_bytes, /* dry_run */ false).unwrap();
        for &(q, prefix) in q {
            let q = db.queue(q).unwrap();
            assert_eq!(q.len(), 1);
            if let Some(prefix) = prefix {
                assert_eq!(
                    q[0].comment.as_ref().and_then(|c| c.get(..prefix.len())),
                    Some(prefix)
                );
            }
        }
    }
}
