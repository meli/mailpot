use mailpot::{models::*, Configuration, Database, SendMail};
use tempfile::TempDir;

#[test]
fn test_init_empty() {
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
}

#[test]
fn test_list_creation() {
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
}
