/*
 * This file is part of mailpot
 *
 * Copyright 2023 - Manos Pitsidianakis
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

use jsonschema::JSONSchema;
use mailpot::{Configuration, Connection, SendMail};
use mailpot_tests::init_stderr_logging;
use serde_json::{json, Value};
use tempfile::TempDir;

#[test]
fn test_settings_json_schemas_are_valid() {
    init_stderr_logging();
    let tmp_dir = TempDir::new().unwrap();

    let db_path = tmp_dir.path().join("mpot.db");
    std::fs::copy("../mailpot-tests/for_testing.db", &db_path).unwrap();
    let mut perms = std::fs::metadata(&db_path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&db_path, perms).unwrap();

    let config = Configuration {
        send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
        administrators: vec![],
    };
    let db = Connection::open_or_create_db(config).unwrap().trusted();

    let schemas: Vec<String> = {
        let mut stmt = db
            .connection
            .prepare("SELECT value FROM list_settings_json;")
            .unwrap();
        let iter = stmt
            .query_map([], |row| {
                let value: String = row.get("value")?;
                Ok(value)
            })
            .unwrap();
        let mut ret = vec![];
        for item in iter {
            ret.push(item.unwrap());
        }
        ret
    };
    println!("Testing that schemas are valid…");
    for schema in schemas {
        let schema: Value = serde_json::from_str(&schema).unwrap();
        let _compiled = JSONSchema::compile(&schema).expect("A valid schema");
    }
}

#[test]
fn test_settings_json_triggers() {
    init_stderr_logging();
    let tmp_dir = TempDir::new().unwrap();

    let db_path = tmp_dir.path().join("mpot.db");
    std::fs::copy("../mailpot-tests/for_testing.db", &db_path).unwrap();
    let mut perms = std::fs::metadata(&db_path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&db_path, perms).unwrap();

    let config = Configuration {
        send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
        administrators: vec![],
    };
    let db = Connection::open_or_create_db(config).unwrap().trusted();
    let list = db.lists().unwrap().remove(0);

    let archived_at_link_settings_schema =
        std::fs::read_to_string("./settings_json_schemas/archivedatlink.json").unwrap();

    println!("Testing that inserting settings works…");
    let (settings_pk, settings_val, last_modified): (i64, Value, i64) = {
        let mut stmt = db
            .connection
            .prepare(
                "INSERT INTO list_settings_json(name, list, value) \
                 VALUES('ArchivedAtLinkSettings', ?, ?) RETURNING pk, value, last_modified;",
            )
            .unwrap();
        stmt.query_row(
            rusqlite::params![
                &list.pk(),
                &json!({
                    "template": "https://www.example.com/{{msg_id}}.html",
                    "preserve_carets": false
                }),
            ],
            |row| {
                let pk: i64 = row.get("pk")?;
                let value: Value = row.get("value")?;
                let last_modified: i64 = row.get("last_modified")?;
                Ok((pk, value, last_modified))
            },
        )
        .unwrap()
    };
    db.connection
        .execute_batch("UPDATE list_settings_json SET is_valid = 1;")
        .unwrap();

    println!("Testing that schema is actually valid…");
    let schema: Value = serde_json::from_str(&archived_at_link_settings_schema).unwrap();
    let compiled = JSONSchema::compile(&schema).expect("A valid schema");
    if let Err(errors) = compiled.validate(&settings_val) {
        for err in errors {
            eprintln!("Error: {err}");
        }
        panic!("Could not validate settings.");
    };

    println!("Testing that inserting invalid settings aborts…");
    {
        let mut stmt = db
            .connection
            .prepare(
                "INSERT OR REPLACE INTO list_settings_json(name, list, value) \
                 VALUES('ArchivedAtLinkSettings', ?, ?) RETURNING pk, value;",
            )
            .unwrap();
        assert_eq!(
            "new settings value is not valid according to the json schema. Rolling back \
             transaction.",
            &stmt
                .query_row(
                    rusqlite::params![
                        &list.pk(),
                        &json!({
                            "template": "https://www.example.com/msg-id}.html" // should be msg_id
                        }),
                    ],
                    |row| {
                        let pk: i64 = row.get("pk")?;
                        let value: Value = row.get("value")?;
                        Ok((pk, value))
                    },
                )
                .unwrap_err()
                .to_string()
        );
    };

    println!("Testing that updating settings with invalid value aborts…");
    {
        let mut stmt = db
            .connection
            .prepare(
                "UPDATE list_settings_json SET value = ? WHERE name = 'ArchivedAtLinkSettings' \
                 RETURNING pk, value;",
            )
            .unwrap();
        assert_eq!(
            "new settings value is not valid according to the json schema. Rolling back \
             transaction.",
            &stmt
                .query_row(
                    rusqlite::params![&json!({
                        "template": "https://www.example.com/msg-id}.html" // should be msg_id
                    }),],
                    |row| {
                        let pk: i64 = row.get("pk")?;
                        let value: Value = row.get("value")?;
                        Ok((pk, value))
                    },
                )
                .unwrap_err()
                .to_string()
        );
    };

    std::thread::sleep(std::time::Duration::from_millis(1000));
    println!("Finally, testing that updating schema reverifies settings…");
    {
        let mut stmt = db
            .connection
            .prepare(
                "UPDATE settings_json_schema SET id = ? WHERE id = 'ArchivedAtLinkSettings' \
                 RETURNING pk;",
            )
            .unwrap();
        stmt.query_row([&"ArchivedAtLinkSettingsv2"], |_| Ok(()))
            .unwrap();
    };
    let (new_name, is_valid, new_last_modified): (String, bool, i64) = {
        let mut stmt = db
            .connection
            .prepare("SELECT name, is_valid, last_modified from list_settings_json WHERE pk = ?;")
            .unwrap();
        stmt.query_row([&settings_pk], |row| {
            Ok((
                row.get("name")?,
                row.get("is_valid")?,
                row.get("last_modified")?,
            ))
        })
        .unwrap()
    };
    assert_eq!(&new_name, "ArchivedAtLinkSettingsv2");
    assert!(is_valid);
    assert!(new_last_modified != last_modified);
}
