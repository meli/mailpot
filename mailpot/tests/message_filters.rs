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
use serde_json::json;
use tempfile::TempDir;

#[test]
fn test_post_filters() {
    init_stderr_logging();
    let tmp_dir = TempDir::new().unwrap();

    let mut post_policy = PostPolicy {
        pk: -1,
        list: -1,
        announce_only: false,
        subscription_only: false,
        approval_needed: false,
        open: true,
        custom: false,
    };
    let db_path = tmp_dir.path().join("mpot.db");
    let config = Configuration {
        send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
        administrators: vec![],
    };

    let db = Connection::open_or_create_db(config).unwrap().trusted();
    let foo_chat = db
        .create_list(MailingList {
            pk: 0,
            name: "foobar chat".into(),
            id: "foo-chat".into(),
            address: "foo-chat@example.com".into(),
            description: None,
            topics: vec![],
            archive_url: None,
        })
        .unwrap();
    post_policy.list = foo_chat.pk();
    db.add_subscription(
        foo_chat.pk(),
        ListSubscription {
            pk: -1,
            list: foo_chat.pk(),
            address: "user@example.com".into(),
            name: None,
            account: None,
            digest: false,
            enabled: true,
            verified: true,
            hide_address: false,
            receive_duplicates: true,
            receive_own_posts: true,
            receive_confirmation: false,
        },
    )
    .unwrap();
    db.set_list_post_policy(post_policy).unwrap();

    println!("Check that List subject prefix is inserted and can be optionally disabled…");
    let post_bytes = b"From: Name <user@example.com>\x0D
To: <foo-chat@example.com>\x0D
Subject: This is a post\x0D
Date: Thu, 29 Oct 2020 13:58:16 +0000\x0D
Message-ID: <abcdefgh@sator.example.com>\x0D
Content-Language: en-US\x0D
Content-Type: text/html\x0D
Content-Transfer-Encoding: base64\x0D
MIME-Version: 1.0\x0D
\x0D
PCFET0NUWVBFPjxodG1sPjxoZWFkPjx0aXRsZT5mb288L3RpdGxlPjwvaGVhZD48Ym9k\x0D
eT48dGFibGUgY2xhc3M9ImZvbyI+PHRoZWFkPjx0cj48dGQ+Zm9vPC90ZD48L3RoZWFk\x0D
Pjx0Ym9keT48dHI+PHRkPmZvbzE8L3RkPjwvdHI+PC90Ym9keT48L3RhYmxlPjwvYm9k\x0D
eT48L2h0bWw+";
    let envelope = melib::Envelope::from_bytes(post_bytes, None).expect("Could not parse message");
    db.post(&envelope, post_bytes, /* dry_run */ false).unwrap();
    let q = db.queue(Queue::Out).unwrap();
    assert_eq!(&q[0].subject, "[foo-chat] This is a post");
    let q_env = melib::Envelope::from_bytes(&q[0].message, None).expect("Could not parse message");
    assert_eq!(
        String::from_utf8_lossy(envelope.body_bytes(post_bytes).body()),
        String::from_utf8_lossy(q_env.body_bytes(&q[0].message).body()),
        "Post body was malformed by message filters!"
    );

    db.delete_from_queue(Queue::Out, vec![]).unwrap();
    {
        let mut stmt = db
            .connection
            .prepare(
                "INSERT INTO list_settings_json(name, list, value) \
                 VALUES('AddSubjectTagPrefixSettings', ?, ?) RETURNING *;",
            )
            .unwrap();
        stmt.query_row(
            rusqlite::params![
                &foo_chat.pk(),
                &json!({
                    "enabled": false
                }),
            ],
            |_| Ok(()),
        )
        .unwrap();
    }
    db.post(&envelope, post_bytes, /* dry_run */ false).unwrap();
    let q = db.queue(Queue::Out).unwrap();
    assert_eq!(&q[0].subject, "This is a post");
    db.delete_from_queue(Queue::Out, vec![]).unwrap();

    println!("Check that List headers are encoded with MIME when necessary…");
    db.update_list(changesets::MailingListChangeset {
        pk: foo_chat.pk,
        description: Some(Some(
            "Why, I, in this weak piping time of peace,\nHave no delight to pass away the \
             time,\nUnless to spy my shadow in the sun."
                .to_string(),
        )),
        ..Default::default()
    })
    .unwrap();
    db.post(&envelope, post_bytes, /* dry_run */ false).unwrap();
    let q = db.queue(Queue::Out).unwrap();
    let q_env = melib::Envelope::from_bytes(&q[0].message, None).expect("Could not parse message");
    assert_eq!(
        String::from_utf8_lossy(envelope.body_bytes(post_bytes).body()),
        String::from_utf8_lossy(q_env.body_bytes(&q[0].message).body()),
        "Post body was malformed by message filters!"
    );
    assert_eq!(
        &q_env.other_headers[melib::HeaderName::LIST_ID],
        "Why, I, in this weak piping time of peace,\nHave no delight to pass away the \
         time,\nUnless to spy my shadow in the sun. <foo-chat.example.com>"
    );
    db.delete_from_queue(Queue::Out, vec![]).unwrap();

    db.update_list(changesets::MailingListChangeset {
        pk: foo_chat.pk,
        description: Some(Some(
            r#"<p>Discussion about mailpot, a mailing list manager software.</p>


<ul>
<li>Main git repository: <a href="https://git.meli-email.org/meli/mailpot">https://git.meli-email.org/meli/mailpot</a></li>
<li>Mirror: <a href="https://github.com/meli/mailpot/">https://github.com/meli/mailpot/</a></li>
</ul>"#
                .to_string(),
        )),
        ..Default::default()
    })
    .unwrap();
    db.post(&envelope, post_bytes, /* dry_run */ false).unwrap();
    let q = db.queue(Queue::Out).unwrap();
    let q_env = melib::Envelope::from_bytes(&q[0].message, None).expect("Could not parse message");
    assert_eq!(
        String::from_utf8_lossy(envelope.body_bytes(post_bytes).body()),
        String::from_utf8_lossy(q_env.body_bytes(&q[0].message).body()),
        "Post body was malformed by message filters!"
    );
    assert_eq!(
        &q_env.other_headers[melib::HeaderName::LIST_ID],
        "<p>Discussion about mailpot, a mailing list manager software.</p>\n\n\n<ul>\n<li>Main git repository: <a href=\"https://git.meli-email.org/meli/mailpot\">https://git.meli-email.org/meli/mailpot</a></li>\n<li>Mirror: <a href=\"https://github.com/meli/mailpot/\">https://github.com/meli/mailpot/</a></li>\n</ul> <foo-chat.example.com>"
    );
}
