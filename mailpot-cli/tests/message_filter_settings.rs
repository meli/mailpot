//
// This file is part of mailpot
//
// Copyright 2020 - Manos Pitsidianakis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use assert_cmd::assert::OutputAssertExt;
use mailpot::{Configuration, Connection, SendMail};
use mailpot_tests::*;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_message_filter_settings_command() {
    use assert_cmd::Command;

    let tmp_dir = TempDir::new().unwrap();

    let conf_path = tmp_dir.path().join("conf.toml");
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
    let db = Connection::open_or_create_db(config.clone())
        .unwrap()
        .trusted();
    let list = db.lists().unwrap().remove(0);

    let config_str = config.to_toml();

    std::fs::write(&conf_path, config_str.as_bytes()).unwrap();

    println!("Test print-message-filter-settings --show-available");
    let mut cmd = Command::cargo_bin("mpot").unwrap();
    let output = cmd
        .arg("-vv")
        .arg("-c")
        .arg(&conf_path)
        .arg("list")
        .arg(&list.id)
        .arg("print-message-filter-settings")
        .arg("--show-available")
        .output()
        .unwrap()
        .assert();
    output.code(0).stderr(predicates::str::is_empty()).stdout(
        predicate::eq("AddSubjectTagPrefixSettings\nArchivedAtLinkSettings\nMimeRejectSettings")
            .trim()
            .normalize(),
    );

    println!("Testing that inserting settings works…");
    let mut cmd = Command::cargo_bin("mpot").unwrap();
    let output = cmd
        .arg("-vv")
        .arg("-c")
        .arg(&conf_path)
        .arg("list")
        .arg(&list.id)
        .arg("set-message-filter-setting")
        .arg("--name")
        .arg("ArchivedAtLinkSettings")
        .arg("--value")
        .arg(
            serde_json::to_string_pretty(&serde_json::json!({
                "template": "https://www.example.com/{{msg_id}}.html",
                "preserve_carets": false
            }))
            .unwrap(),
        )
        .output()
        .unwrap()
        .assert();
    output.code(0).stderr(predicates::str::is_empty()).stdout(
        predicate::eq("Successfully updated ArchivedAtLinkSettings value for list foobar chat.")
            .trim()
            .normalize(),
    );

    println!("Test print-message-filter-settings returns values");
    let mut cmd = Command::cargo_bin("mpot").unwrap();
    let output = cmd
        .arg("-vv")
        .arg("-c")
        .arg(&conf_path)
        .arg("list")
        .arg(&list.id)
        .arg("print-message-filter-settings")
        .output()
        .unwrap()
        .assert();
    output.code(0).stderr(predicates::str::is_empty()).stdout(
        predicate::eq("ArchivedAtLinkSettings: {\"preserve_carets\":false,\"template\":\"https://www.example.com/{{msg_id}}.html\"}")
            .trim()
            .normalize(),
    );

    println!("Test print-message-filter-settings returns filtered values");
    let mut cmd = Command::cargo_bin("mpot").unwrap();
    let output = cmd
        .arg("-vv")
        .arg("-c")
        .arg(&conf_path)
        .arg("list")
        .arg(&list.id)
        .arg("print-message-filter-settings")
        .arg("--filter")
        .arg("archived")
        .output()
        .unwrap()
        .assert();
    output.code(0).stderr(predicates::str::is_empty()).stdout(
        predicate::eq("ArchivedAtLinkSettings: {\"preserve_carets\":false,\"template\":\"https://www.example.com/{{msg_id}}.html\"}")
            .trim()
            .normalize(),
    );
    let mut cmd = Command::cargo_bin("mpot").unwrap();
    let output = cmd
        .arg("-vv")
        .arg("-c")
        .arg(&conf_path)
        .arg("list")
        .arg(&list.id)
        .arg("print-message-filter-settings")
        .arg("--filter")
        .arg("aadfa")
        .output()
        .unwrap()
        .assert();
    output
        .code(0)
        .stderr(predicates::str::is_empty())
        .stdout(predicate::eq("").trim().normalize());
}
