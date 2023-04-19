/*
 * meli - email module
 *
 * Copyright 2019 Manos Pitsidianakis
 *
 * This file is part of meli.
 *
 * meli is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * meli is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with meli. If not, see <http://www.gnu.org/licenses/>.
 */

use assert_cmd::assert::OutputAssertExt;
use mailpot::{
    models::{changesets::ListSubscriptionChangeset, *},
    Configuration, Connection, Queue, SendMail,
};
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_out_queue_flush() {
    use assert_cmd::Command;

    let tmp_dir = TempDir::new().unwrap();

    let conf_path = tmp_dir.path().join("conf.toml");
    let db_path = tmp_dir.path().join("mpot.db");

    let config = Configuration {
        send_mail: SendMail::ShellCommand("/usr/bin/true".to_string()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
    };

    let config_str = config.to_toml();

    std::fs::write(&conf_path, config_str.as_bytes()).unwrap();

    let foo_chat = {
        let db = Connection::open_or_create_db(config.clone())
            .unwrap()
            .trusted();

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
        let _post_policy = db
            .set_list_post_policy(PostPolicy {
                pk: -1,
                list: foo_chat.pk(),
                announce_only: false,
                subscription_only: false,
                approval_needed: false,
                open: true,
                custom: false,
            })
            .unwrap();
        foo_chat
    };

    let mut cmd = Command::cargo_bin("mpot").unwrap();
    let output = cmd
        .arg("-vv")
        .arg("-c")
        .arg(&conf_path)
        .arg("flush-queue")
        .output()
        .unwrap()
        .assert();
    output.code(0).stderr(predicates::str::is_empty()).stdout(
        predicate::eq("Queue out has 0 messages.")
            .trim()
            .normalize(),
    );

    fn generate_mail(from: &str, to: &str, subject: &str, body: &str, seq: &mut usize) -> String {
        format!(
            "From: {from}@example.com
To: <foo-chat{to}@example.com>
Subject: {subject}
Date: Thu, 29 Oct 2020 13:58:16 +0000
Message-ID:
 <aaa{}@example.com>
Content-Language: en-US
Content-Type: text/plain

{body}
",
            {
                let val = *seq;
                *seq += 1;
                val
            }
        )
    }

    {
        let mut db = Connection::open_or_create_db(config.clone())
            .unwrap()
            .trusted();

        let mut seq = 0;
        for who in ["Αλίκη", "Χαραλάμπης"] {
            // = ["Alice", "Bob"]
            let mail = generate_mail(who, "+request", "subscribe", "", &mut seq);
            let subenvelope = mailpot::melib::Envelope::from_bytes(mail.as_bytes(), None)
                .expect("Could not parse message");
            db.post(&subenvelope, mail.as_bytes(), /* dry_run */ false)
                .unwrap();
        }
        db.update_subscription(ListSubscriptionChangeset {
            list: foo_chat.pk(),
            address: "Χαραλάμπης@example.com".into(),
            receive_own_posts: Some(true),
            ..Default::default()
        })
        .unwrap();
        let out_queue = db.queue(Queue::Out).unwrap();
        assert_eq!(out_queue.len(), 2);
        assert_eq!(db.list_subscriptions(foo_chat.pk()).unwrap().len(), 2);
        assert_eq!(db.error_queue().unwrap().len(), 0);
        let mail = generate_mail("Χαραλάμπης", "", "hello world", "Hello there.", &mut seq);
        let subenvelope = mailpot::melib::Envelope::from_bytes(mail.as_bytes(), None)
            .expect("Could not parse message");
        db.post(&subenvelope, mail.as_bytes(), /* dry_run */ false)
            .unwrap();
        let out_queue = db.queue(Queue::Out).unwrap();
        assert_eq!(out_queue.len(), 4);
    }

    // [ref:TODO] hook smtp dev server.
    let mut cmd = Command::cargo_bin("mpot").unwrap();
    let output = cmd
        .arg("-vv")
        .arg("-c")
        .arg(&conf_path)
        .arg("flush-queue")
        .output()
        .unwrap()
        .assert();
    output.code(0).stderr(predicates::str::is_empty()).stdout(
        predicate::eq("Queue out has 4 messages.")
            .trim()
            .normalize(),
    );

    {
        let db = Connection::open_or_create_db(config).unwrap().trusted();

        let out_queue = db.queue(Queue::Out).unwrap();
        assert_eq!(out_queue.len(), 0);
    }
}
