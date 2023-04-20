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

mod utils;

use mailpot::{models::*, Configuration, Connection, Queue, SendMail, Template};
use tempfile::TempDir;

#[test]
fn test_template_replies() {
    utils::init_stderr_logging();

    let tmp_dir = TempDir::new().unwrap();

    let db_path = tmp_dir.path().join("mpot.db");
    let config = Configuration {
        send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
        administrators: vec![],
    };

    let mut db = Connection::open_or_create_db(config).unwrap().trusted();
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
    assert_eq!(db.error_queue().unwrap().len(), 0);
    assert_eq!(db.list_subscriptions(foo_chat.pk()).unwrap().len(), 0);

    /* create custom subscribe confirm template, and check that it is used in
     * action */
    let _templ = db
        .add_template(Template {
            pk: 0,
            name: Template::SUBSCRIPTION_CONFIRMATION.into(),
            list: None,
            subject: Some("You have subscribed to {{ list.name }}".into()),
            headers_json: None,
            body: "You have subscribed to {{ list.name }}".into(),
        })
        .unwrap();

    /* subscribe first */

    let bytes = b"From: Name <user@example.com>
To: <foo-chat+subscribe@example.com>
Subject: subscribe
Date: Thu, 29 Oct 2020 13:58:16 +0000
Message-ID:
 <PS1PR0601MB36750BD00EA89E1482FA98A2D5140_2@PS1PR0601MB3675.apcprd06.example.com>
Content-Language: en-US
Content-Type: text/html
Content-Transfer-Encoding: base64
MIME-Version: 1.0

";
    let subenvelope = melib::Envelope::from_bytes(bytes, None).expect("Could not parse message");
    db.post(&subenvelope, bytes, /* dry_run */ false).unwrap();
    assert_eq!(db.list_subscriptions(foo_chat.pk()).unwrap().len(), 1);
    assert_eq!(db.error_queue().unwrap().len(), 0);

    let out_queue = db.queue(Queue::Out).unwrap();
    assert_eq!(out_queue.len(), 1);
    let out = &out_queue[0];
    let out_env = melib::Envelope::from_bytes(&out.message, None).unwrap();
    // eprintln!("{}", String::from_utf8_lossy(&out_bytes));
    assert_eq!(
        &out_env.from()[0].get_email(),
        "foo-chat+request@example.com",
    );
    assert_eq!(
        (
            out_env.to()[0].get_display_name().as_deref(),
            out_env.to()[0].get_email().as_str()
        ),
        (Some("Name"), "user@example.com"),
    );
    assert_eq!(
        &out.subject,
        &format!("You have subscribed to {}", foo_chat.name)
    );

    /* then unsubscribe, remove custom template and subscribe again */

    let unbytes = b"From: Name <user@example.com>
To: <foo-chat+request@example.com>
Subject: unsubscribe
Date: Thu, 29 Oct 2020 13:58:17 +0000
Message-ID:
 <PS1PR0601MB36750BD00EA89E1482FA98A2D5140_3@PS1PR0601MB3675.apcprd06.example.com>
Content-Language: en-US
Content-Type: text/html
Content-Transfer-Encoding: base64
MIME-Version: 1.0

";
    let envelope = melib::Envelope::from_bytes(unbytes, None).expect("Could not parse message");
    db.post(&envelope, unbytes, /* dry_run */ false).unwrap();
    assert_eq!(db.list_subscriptions(foo_chat.pk()).unwrap().len(), 0);
    assert_eq!(db.error_queue().unwrap().len(), 0);

    let out_queue = db.queue(Queue::Out).unwrap();
    assert_eq!(out_queue.len(), 2);

    let mut _templ = _templ.into_inner();
    let _templ2 = db
        .remove_template(Template::SUBSCRIPTION_CONFIRMATION, None)
        .unwrap();
    _templ.pk = _templ2.pk;
    assert_eq!(_templ, _templ2);

    /* now this template should be used: */
    // let default_templ = Template::default_subscription_confirmation();

    db.post(&subenvelope, bytes, /* dry_run */ false).unwrap();
    assert_eq!(db.list_subscriptions(foo_chat.pk()).unwrap().len(), 1);
    assert_eq!(db.error_queue().unwrap().len(), 0);

    let out_queue = db.queue(Queue::Out).unwrap();

    assert_eq!(out_queue.len(), 3);
    let out = &out_queue[2];
    let out_env = melib::Envelope::from_bytes(&out.message, None).unwrap();
    // eprintln!("{}", String::from_utf8_lossy(&out_bytes));
    assert_eq!(
        &out_env.from()[0].get_email(),
        "foo-chat+request@example.com",
    );
    assert_eq!(
        (
            out_env.to()[0].get_display_name().as_deref(),
            out_env.to()[0].get_email().as_str()
        ),
        (Some("Name"), "user@example.com"),
    );
    assert_eq!(
        &out.subject,
        &format!(
            "[{}] You have successfully subscribed to {}.",
            foo_chat.id, foo_chat.name
        )
    );
}
