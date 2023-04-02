use mailpot::{models::*, Configuration, Database, SendMail};
use tempfile::TempDir;

#[test]
fn test_list_subscription() {
    let tmp_dir = TempDir::new().unwrap();

    let db_path = tmp_dir.path().join("mpot.db");
    let config = Configuration {
        send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        db_path: db_path.clone(),
        storage: "sqlite3".to_string(),
        data_path: tmp_dir.path().to_path_buf(),
    };

    let db = Database::open_or_create_db(&config).unwrap();
    assert!(db.list_lists().unwrap().is_empty());
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
    let lists = db.list_lists().unwrap();
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0], foo_chat);
    let post_policy = db
        .set_list_policy(
            foo_chat.pk(),
            PostPolicy {
                pk: 0,
                list: foo_chat.pk(),
                announce_only: false,
                subscriber_only: true,
                approval_needed: false,
                no_subscriptions: false,
                custom: false,
            },
        )
        .unwrap();

    assert_eq!(post_policy.pk(), 1);
    assert_eq!(db.error_queue().unwrap().len(), 0);
    let input_bytes_1 = b"From: Name <user@example.com>
To: <foo-chat@example.com>
Subject: This is a post
Date: Thu, 29 Oct 2020 13:58:16 +0000
Message-ID:
 <PS1PR0601MB36750BD00EA89E1482FA98A2D5140@PS1PR0601MB3675.apcprd06.prod.outlook.com>
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
        melib::Envelope::from_bytes(input_bytes_1, None).expect("Could not parse message");
    match db
        .post(&envelope, input_bytes_1, /* dry_run */ false)
        .unwrap_err()
        .kind()
    {
        mailpot::ErrorKind::PostRejected(_reason) => {}
        other => panic!("Got unexpected error: {}", other),
    }
    assert_eq!(db.error_queue().unwrap().len(), 1);

    let input_bytes_2 = b"From: Name <user@example.com>
To: <foo-chat+subscribe@example.com>
Subject: subscribe
Date: Thu, 29 Oct 2020 13:58:16 +0000
Message-ID:
 <PS1PR0601MB36750BD00EA89E1482FA98A2D5140_2@PS1PR0601MB3675.apcprd06.prod.outlook.com>
Content-Language: en-US
Content-Type: text/html
Content-Transfer-Encoding: base64
MIME-Version: 1.0

";
    let envelope =
        melib::Envelope::from_bytes(input_bytes_2, None).expect("Could not parse message");
    db.post(&envelope, input_bytes_2, /* dry_run */ false)
        .unwrap();
    assert_eq!(db.error_queue().unwrap().len(), 1);
    let envelope =
        melib::Envelope::from_bytes(input_bytes_1, None).expect("Could not parse message");
    db.post(&envelope, input_bytes_1, /* dry_run */ false)
        .unwrap();
    assert_eq!(db.error_queue().unwrap().len(), 1);
    assert_eq!(db.list_posts(foo_chat.pk(), None).unwrap().len(), 1);
}
