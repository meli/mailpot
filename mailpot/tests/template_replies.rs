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

use mailpot::{models::*, queue::Queue, Configuration, Connection, SendMail, Template};
use mailpot_tests::init_stderr_logging;
use tempfile::TempDir;

#[test]
fn test_template_replies() {
    init_stderr_logging();

    const SUB_BYTES: &[u8] = b"From: Name <user@example.com>
To: <foo-chat+subscribe@example.com>
Subject: subscribe
Date: Thu, 29 Oct 2020 13:58:16 +0000
Message-ID: <abcdefgh@sator.example.com>
Content-Language: en-US
Content-Type: text/html
Content-Transfer-Encoding: base64
MIME-Version: 1.0

";
    const UNSUB_BYTES: &[u8] = b"From: Name <user@example.com>
To: <foo-chat+request@example.com>
Subject: unsubscribe
Date: Thu, 29 Oct 2020 13:58:17 +0000
Message-ID: <abcdefgh@sator.example.com>
Content-Language: en-US
Content-Type: text/html
Content-Transfer-Encoding: base64
MIME-Version: 1.0

";

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
            topics: vec![],
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

    let _templ_gen = db
        .add_template(Template {
            pk: -1,
            name: Template::SUBSCRIPTION_CONFIRMATION.into(),
            list: None,
            subject: Some("You have subscribed to a list".into()),
            headers_json: None,
            body: "You have subscribed to a list".into(),
        })
        .unwrap();
    /* create custom subscribe confirm template, and check that it is used in
     * action */
    let _templ = db
        .add_template(Template {
            pk: -1,
            name: Template::SUBSCRIPTION_CONFIRMATION.into(),
            list: Some(foo_chat.pk()),
            subject: Some("You have subscribed to {{ list.name }}".into()),
            headers_json: None,
            body: "You have subscribed to {{ list.name }}".into(),
        })
        .unwrap();
    let _all = db.fetch_templates().unwrap();
    assert_eq!(&_all[0], &_templ_gen);
    assert_eq!(&_all[1], &_templ);
    assert_eq!(_all.len(), 2);

    let sub_fn = |db: &mut Connection| {
        let subenvelope =
            melib::Envelope::from_bytes(SUB_BYTES, None).expect("Could not parse message");
        db.post(&subenvelope, SUB_BYTES, /* dry_run */ false)
            .unwrap();
        assert_eq!(db.list_subscriptions(foo_chat.pk()).unwrap().len(), 1);
        assert_eq!(db.queue(Queue::Error).unwrap().len(), 0);
    };
    let unsub_fn = |db: &mut Connection| {
        let envelope =
            melib::Envelope::from_bytes(UNSUB_BYTES, None).expect("Could not parse message");
        db.post(&envelope, UNSUB_BYTES, /* dry_run */ false)
            .unwrap();
        assert_eq!(db.list_subscriptions(foo_chat.pk()).unwrap().len(), 0);
        assert_eq!(db.queue(Queue::Error).unwrap().len(), 0);
    };

    /* subscribe first */

    sub_fn(&mut db);

    let out_queue = db.queue(Queue::Out).unwrap();
    assert_eq!(out_queue.len(), 1);
    let out = &out_queue[0];
    let out_env = melib::Envelope::from_bytes(&out.message, None).unwrap();

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

    unsub_fn(&mut db);

    let out_queue = db.queue(Queue::Out).unwrap();
    assert_eq!(out_queue.len(), 2);

    let mut _templ = _templ.into_inner();
    let _templ2 = db
        .remove_template(Template::SUBSCRIPTION_CONFIRMATION, Some(foo_chat.pk()))
        .unwrap();
    _templ.pk = _templ2.pk;
    assert_eq!(_templ, _templ2);

    /* now the first inserted template should be used: */

    sub_fn(&mut db);

    let out_queue = db.queue(Queue::Out).unwrap();

    assert_eq!(out_queue.len(), 3);
    let out = &out_queue[2];
    let out_env = melib::Envelope::from_bytes(&out.message, None).unwrap();

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
    assert_eq!(&out.subject, "You have subscribed to a list");

    unsub_fn(&mut db);
    let mut _templ_gen_2 = db
        .remove_template(Template::SUBSCRIPTION_CONFIRMATION, None)
        .unwrap();
    _templ_gen_2.pk = _templ_gen.pk;
    assert_eq!(_templ_gen_2, _templ_gen.into_inner());

    /* now this template should be used: */

    sub_fn(&mut db);

    let out_queue = db.queue(Queue::Out).unwrap();

    assert_eq!(out_queue.len(), 5);
    let out = &out_queue[4];
    let out_env = melib::Envelope::from_bytes(&out.message, None).unwrap();

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
