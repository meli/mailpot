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

use log::{trace, warn};
use mailpot::{melib, models::*, queue::Queue, Configuration, Connection, SendMail};
use mailpot_tests::*;
use melib::smol;
use tempfile::TempDir;

#[test]
fn test_smtp() {
    init_stderr_logging();

    let tmp_dir = TempDir::new().unwrap();

    let smtp_handler = TestSmtpHandler::builder().address("127.0.0.1:8825").build();

    let db_path = tmp_dir.path().join("mpot.db");
    let config = Configuration {
        send_mail: SendMail::Smtp(smtp_handler.smtp_conf()),
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
            topics: vec![],
            archive_url: None,
        })
        .unwrap();

    assert_eq!(foo_chat.pk(), 1);
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

    let input_bytes = include_bytes!("./test_sample_longmessage.eml");
    match melib::Envelope::from_bytes(input_bytes, None) {
        Ok(envelope) => {
            // eprintln!("envelope {:?}", &envelope);
            db.post(&envelope, input_bytes, /* dry_run */ false)
                .expect("Got unexpected error");
            {
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
            }

            db.add_subscription(
                foo_chat.pk(),
                ListSubscription {
                    pk: 0,
                    list: foo_chat.pk(),
                    address: "paaoejunp@example.com".into(),
                    name: Some("Cardholder Name".into()),
                    account: None,
                    digest: false,
                    verified: true,
                    hide_address: false,
                    receive_duplicates: true,
                    receive_own_posts: true,
                    receive_confirmation: true,
                    enabled: true,
                },
            )
            .unwrap();
            db.add_subscription(
                foo_chat.pk(),
                ListSubscription {
                    pk: 0,
                    list: foo_chat.pk(),
                    address: "manos@example.com".into(),
                    name: Some("Manos Hands".into()),
                    account: None,
                    digest: false,
                    verified: true,
                    hide_address: false,
                    receive_duplicates: true,
                    receive_own_posts: true,
                    receive_confirmation: true,
                    enabled: true,
                },
            )
            .unwrap();
            db.post(&envelope, input_bytes, /* dry_run */ false)
                .unwrap();
        }
        Err(err) => {
            panic!("Could not parse message: {}", err);
        }
    }
    let messages = db.delete_from_queue(Queue::Out, vec![]).unwrap();
    eprintln!("Queue out has {} messages.", messages.len());
    let conn_future = db.new_smtp_connection().unwrap();
    smol::future::block_on(smol::spawn(async move {
        let mut conn = conn_future.await.unwrap();
        for msg in messages {
            Connection::submit(&mut conn, &msg, /* dry_run */ false)
                .await
                .unwrap();
        }
    }));
    let stored = smtp_handler.stored.lock().unwrap();
    assert_eq!(stored.len(), 3);
    assert_eq!(&stored[0].0, "paaoejunp@example.com");
    assert_eq!(
        &stored[0].1.subject(),
        "Your post to foo-chat was rejected."
    );
    assert_eq!(
        &stored[1].1.subject(),
        "[foo-chat] thankful that I had the chance to written report, that I could learn and let \
         alone the chance $4454.32"
    );
    assert_eq!(
        &stored[2].1.subject(),
        "[foo-chat] thankful that I had the chance to written report, that I could learn and let \
         alone the chance $4454.32"
    );
}

#[test]
fn test_smtp_mailcrab() {
    use std::env;
    init_stderr_logging();

    fn get_smtp_conf() -> melib::smtp::SmtpServerConf {
        use melib::smtp::*;
        SmtpServerConf {
            hostname: "127.0.0.1".into(),
            port: 1025,
            envelope_from: "foo-chat@example.com".into(),
            auth: SmtpAuth::None,
            security: SmtpSecurity::None,
            extensions: Default::default(),
        }
    }

    let Ok(mailcrab_ip) = env::var("MAILCRAB_IP") else {
        warn!("MAILCRAB_IP env var not set, is mailcrab server running?");
        return;
    };
    let mailcrab_port = env::var("MAILCRAB_PORT").unwrap_or("1080".to_string());
    let api_uri = format!("http://{mailcrab_ip}:{mailcrab_port}/api/messages");

    let tmp_dir = TempDir::new().unwrap();

    let db_path = tmp_dir.path().join("mpot.db");
    let config = Configuration {
        send_mail: SendMail::Smtp(get_smtp_conf()),
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
            topics: vec![],
            archive_url: None,
        })
        .unwrap();

    assert_eq!(foo_chat.pk(), 1);
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

    let input_bytes = include_bytes!("./test_sample_longmessage.eml");
    match melib::Envelope::from_bytes(input_bytes, None) {
        Ok(envelope) => {
            match db
                .post(&envelope, input_bytes, /* dry_run */ false)
                .unwrap_err()
                .kind()
            {
                mailpot::PostRejected(reason) => {
                    trace!("Non-subscription post succesfully rejected: '{reason}'");
                }
                other => panic!("Got unexpected error: {}", other),
            }
            db.add_subscription(
                foo_chat.pk(),
                ListSubscription {
                    pk: 0,
                    list: foo_chat.pk(),
                    address: "paaoejunp@example.com".into(),
                    name: Some("Cardholder Name".into()),
                    account: None,
                    digest: false,
                    verified: true,
                    hide_address: false,
                    receive_duplicates: true,
                    receive_own_posts: true,
                    receive_confirmation: true,
                    enabled: true,
                },
            )
            .unwrap();
            db.add_subscription(
                foo_chat.pk(),
                ListSubscription {
                    pk: 0,
                    list: foo_chat.pk(),
                    address: "manos@example.com".into(),
                    name: Some("Manos Hands".into()),
                    account: None,
                    digest: false,
                    verified: true,
                    hide_address: false,
                    receive_duplicates: true,
                    receive_own_posts: true,
                    receive_confirmation: true,
                    enabled: true,
                },
            )
            .unwrap();
            db.post(&envelope, input_bytes, /* dry_run */ false)
                .unwrap();
        }
        Err(err) => {
            panic!("Could not parse message: {}", err);
        }
    }
    let mails: String = reqwest::blocking::get(api_uri).unwrap().text().unwrap();
    trace!("mails: {}", mails);
}
