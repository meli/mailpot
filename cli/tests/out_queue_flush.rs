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
    melib,
    models::{changesets::ListSubscriptionChangeset, *},
    Configuration, Connection, Queue, SendMail,
};
use mailpot_tests::*;
use predicates::prelude::*;
use tempfile::TempDir;

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

#[test]
fn test_out_queue_flush() {
    use assert_cmd::Command;

    let tmp_dir = TempDir::new().unwrap();

    let conf_path = tmp_dir.path().join("conf.toml");
    let db_path = tmp_dir.path().join("mpot.db");
    let smtp_handler = TestSmtpHandler::builder().address("127.0.0.1:8826").build();
    let config = Configuration {
        send_mail: SendMail::Smtp(smtp_handler.smtp_conf()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
        administrators: vec![],
    };

    let config_str = config.to_toml();

    std::fs::write(&conf_path, config_str.as_bytes()).unwrap();

    log::info!("Creating foo-chat@example.com mailing list.");
    let post_policy;
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
        post_policy = db
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

    let headers_fn = |env: &melib::Envelope| {
        assert!(env.subject().starts_with(&format!("[{}] ", foo_chat.id)));
        let headers = env.other_headers();

        assert_eq!(headers.get("List-Id"), Some(&foo_chat.id_header()));
        assert_eq!(headers.get("List-Help"), foo_chat.help_header().as_ref());
        assert_eq!(
            headers.get("List-Post"),
            foo_chat.post_header(Some(&post_policy)).as_ref()
        );
    };

    log::info!("Running mpot flush-queue on empty out queue.");
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

    let mut seq = 0; // for generated emails
    log::info!("Subscribe two users, Αλίκη and Χαραλάμπης to foo-chat.");

    {
        let mut db = Connection::open_or_create_db(config.clone())
            .unwrap()
            .trusted();

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
        assert_eq!(db.queue(Queue::Error).unwrap().len(), 0);
    }

    log::info!("Flush out queue, subscription confirmations should be sent to the new users.");
    let mut cmd = Command::cargo_bin("mpot").unwrap();
    let output = cmd
        .arg("-vv")
        .arg("-c")
        .arg(&conf_path)
        .arg("flush-queue")
        .output()
        .unwrap()
        .assert();
    output.code(0).stdout(
        predicate::eq("Queue out has 2 messages.")
            .trim()
            .normalize(),
    );

    /* Check that confirmation emails are correct */
    let stored = std::mem::take(&mut *smtp_handler.stored.lock().unwrap());
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].0, "=?UTF-8?B?zpHOu86vzrrOtw==?=@example.com");
    assert_eq!(
        stored[1].0,
        "=?UTF-8?B?zqfOsc+BzrHOu86szrzPgM63z4I=?=@example.com"
    );
    for item in stored.iter() {
        assert_eq!(
            item.1.subject(),
            "[foo-chat] You have successfully subscribed to foobar chat."
        );
        assert_eq!(
            &item.1.field_from_to_string(),
            "foo-chat+request@example.com"
        );
        headers_fn(&item.1);
    }

    log::info!(
        "Χαραλάμπης submits a post to list. Flush out queue, Χαραλάμπης' post should be relayed \
         to Αλίκη, and Χαραλάμπης should receive a copy of their own post because of \
         `receive_own_posts` setting."
    );

    {
        let mut db = Connection::open_or_create_db(config.clone())
            .unwrap()
            .trusted();
        let mail = generate_mail("Χαραλάμπης", "", "hello world", "Hello there.", &mut seq);
        let subenvelope = mailpot::melib::Envelope::from_bytes(mail.as_bytes(), None)
            .expect("Could not parse message");
        db.post(&subenvelope, mail.as_bytes(), /* dry_run */ false)
            .unwrap();
        let out_queue = db.queue(Queue::Out).unwrap();
        assert_eq!(out_queue.len(), 2);
    }

    let mut cmd = Command::cargo_bin("mpot").unwrap();
    let output = cmd
        .arg("-vv")
        .arg("-c")
        .arg(&conf_path)
        .arg("flush-queue")
        .output()
        .unwrap()
        .assert();
    output.code(0).stdout(
        predicate::eq("Queue out has 2 messages.")
            .trim()
            .normalize(),
    );

    /* Check that user posts are correct */
    {
        let db = Connection::open_or_create_db(config).unwrap().trusted();

        let out_queue = db.queue(Queue::Out).unwrap();
        assert_eq!(out_queue.len(), 0);
    }

    let stored = std::mem::take(&mut *smtp_handler.stored.lock().unwrap());
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].0, "Αλίκη@example.com");
    assert_eq!(stored[1].0, "Χαραλάμπης@example.com");
    assert_eq!(stored[0].1.message_id(), stored[1].1.message_id());
    assert_eq!(stored[0].1.other_headers(), stored[1].1.other_headers());
    headers_fn(&stored[0].1);
}

#[test]
fn test_list_requests_submission() {
    use assert_cmd::Command;

    let tmp_dir = TempDir::new().unwrap();

    let conf_path = tmp_dir.path().join("conf.toml");
    let db_path = tmp_dir.path().join("mpot.db");
    let smtp_handler = TestSmtpHandler::builder().address("127.0.0.1:8827").build();
    let config = Configuration {
        send_mail: SendMail::Smtp(smtp_handler.smtp_conf()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
        administrators: vec![],
    };

    let config_str = config.to_toml();

    std::fs::write(&conf_path, config_str.as_bytes()).unwrap();

    log::info!("Creating foo-chat@example.com mailing list.");
    let post_policy;
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
        post_policy = db
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

    let headers_fn = |env: &melib::Envelope| {
        let headers = env.other_headers();

        assert_eq!(headers.get("List-Id"), Some(&foo_chat.id_header()));
        assert_eq!(headers.get("List-Help"), foo_chat.help_header().as_ref());
        assert_eq!(
            headers.get("List-Post"),
            foo_chat.post_header(Some(&post_policy)).as_ref()
        );
    };

    log::info!("Running mpot flush-queue on empty out queue.");
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

    let mut seq = 0; // for generated emails
    log::info!("User Αλίκη sends to foo-chat+request with subject 'help'.");

    {
        let mut db = Connection::open_or_create_db(config).unwrap().trusted();

        let mail = generate_mail("Αλίκη", "+request", "help", "", &mut seq);
        let subenvelope = mailpot::melib::Envelope::from_bytes(mail.as_bytes(), None)
            .expect("Could not parse message");
        db.post(&subenvelope, mail.as_bytes(), /* dry_run */ false)
            .unwrap();
        let out_queue = db.queue(Queue::Out).unwrap();
        assert_eq!(out_queue.len(), 1);
        assert_eq!(db.list_subscriptions(foo_chat.pk()).unwrap().len(), 0);
        assert_eq!(db.queue(Queue::Error).unwrap().len(), 0);
    }

    log::info!("Flush out queue, help reply should go to Αλίκη.");
    let mut cmd = Command::cargo_bin("mpot").unwrap();
    let output = cmd
        .arg("-vv")
        .arg("-c")
        .arg(&conf_path)
        .arg("flush-queue")
        .output()
        .unwrap()
        .assert();
    output.code(0).stdout(
        predicate::eq("Queue out has 1 messages.")
            .trim()
            .normalize(),
    );

    /* Check that help email is correct */
    let stored = std::mem::take(&mut *smtp_handler.stored.lock().unwrap());
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].0, "=?UTF-8?B?zpHOu86vzrrOtw==?=@example.com");
    assert_eq!(stored[0].1.subject(), "Help for foobar chat");
    assert_eq!(
        &stored[0].1.field_from_to_string(),
        "foo-chat+request@example.com"
    );
    headers_fn(&stored[0].1);
}
