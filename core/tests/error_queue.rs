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

use mailpot::{melib, models::*, queue::Queue, Configuration, Connection, SendMail};
use mailpot_tests::init_stderr_logging;
use tempfile::TempDir;

fn get_smtp_conf() -> melib::smtp::SmtpServerConf {
    use melib::smtp::*;
    SmtpServerConf {
        hostname: "127.0.0.1".into(),
        port: 8825,
        envelope_from: "foo-chat@example.com".into(),
        auth: SmtpAuth::None,
        security: SmtpSecurity::None,
        extensions: Default::default(),
    }
}

#[test]
fn test_error_queue() {
    init_stderr_logging();
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
    assert_eq!(db.queue(Queue::Error).unwrap().len(), 0);

    // drop privileges
    let db = db.untrusted();

    let input_bytes = include_bytes!("./test_sample_longmessage.eml");
    let envelope = melib::Envelope::from_bytes(input_bytes, None).expect("Could not parse message");
    db.post(&envelope, input_bytes, /* dry_run */ false)
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
}
