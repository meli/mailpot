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

use mailpot::{models::*, Configuration, Connection, ErrorKind, SendMail};
use mailpot_tests::init_stderr_logging;
use tempfile::TempDir;

#[test]
fn test_authorizer() {
    init_stderr_logging();
    let tmp_dir = TempDir::new().unwrap();

    let db_path = tmp_dir.path().join("mpot.db");
    let config = Configuration {
        send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
        administrators: vec![],
    };

    let db = Connection::open_or_create_db(config).unwrap();
    assert!(db.lists().unwrap().is_empty());

    for err in [
        db.create_list(MailingList {
            pk: 0,
            name: "foobar chat".into(),
            id: "foo-chat".into(),
            address: "foo-chat@example.com".into(),
            description: None,
            topics: vec![],
            archive_url: None,
        })
        .unwrap_err(),
        db.remove_list_owner(1, 1).unwrap_err(),
        db.remove_list_post_policy(1, 1).unwrap_err(),
        db.set_list_post_policy(PostPolicy {
            pk: 0,
            list: 1,
            announce_only: false,
            subscription_only: true,
            approval_needed: false,
            open: false,
            custom: false,
        })
        .unwrap_err(),
    ] {
        assert_eq!(
            err.kind().to_string(),
            ErrorKind::Sql(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error {
                    code: rusqlite::ErrorCode::AuthorizationForStatementDenied,
                    extended_code: 23,
                },
                Some("not authorized".into()),
            ))
            .to_string()
        );
    }
    assert!(db.lists().unwrap().is_empty());

    let db = db.trusted();

    for ok in [
        db.create_list(MailingList {
            pk: 0,
            name: "foobar chat".into(),
            id: "foo-chat".into(),
            address: "foo-chat@example.com".into(),
            description: None,
            topics: vec![],
            archive_url: None,
        })
        .map(|_| ()),
        db.add_list_owner(ListOwner {
            pk: 0,
            list: 1,
            address: String::new(),
            name: None,
        })
        .map(|_| ()),
        db.set_list_post_policy(PostPolicy {
            pk: 0,
            list: 1,
            announce_only: false,
            subscription_only: true,
            approval_needed: false,
            open: false,
            custom: false,
        })
        .map(|_| ()),
        db.remove_list_post_policy(1, 1).map(|_| ()),
        db.remove_list_owner(1, 1).map(|_| ()),
    ] {
        ok.unwrap();
    }
}
