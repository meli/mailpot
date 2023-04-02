use mailpot::{melib, models::*, Configuration, Database, SendMail};
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
    stderrlog::new()
        .quiet(false)
        .verbosity(15)
        .show_module_names(true)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();
    let tmp_dir = TempDir::new().unwrap();

    let db_path = tmp_dir.path().join("mpot.db");
    let config = Configuration {
        send_mail: SendMail::Smtp(get_smtp_conf()),
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

    let input_bytes = include_bytes!("./test_sample_longmessage.eml");
    let envelope = melib::Envelope::from_bytes(input_bytes, None).expect("Could not parse message");
    match db
        .post(&envelope, input_bytes, /* dry_run */ false)
        .unwrap_err()
        .kind()
    {
        mailpot::ErrorKind::PostRejected(_reason) => {}
        other => panic!("Got unexpected error: {}", other),
    }
    assert_eq!(db.error_queue().unwrap().len(), 1);
}
