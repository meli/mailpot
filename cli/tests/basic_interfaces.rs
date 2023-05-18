/*
 * meli - email module
 *
 * Copyright 2019 Manos Pitsidianakis
 *
 * This file is part of meli.
 *
 * meli is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * meli is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with meli. If not, see <http://www.gnu.org/licenses/>.
 */

#![deny(dead_code)]

use std::path::Path;

use assert_cmd::{assert::OutputAssertExt, Command};
use mailpot::{models::*, Configuration, Connection, SendMail};
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_cli_basic_interfaces() {
    fn no_args() {
        let mut cmd = Command::cargo_bin("mpot").unwrap();
        // 2 -> incorrect usage
        cmd.assert().code(2);
    }

    fn version() {
        // --version is successful
        for arg in ["--version", "-V"] {
            let mut cmd = Command::cargo_bin("mpot").unwrap();
            let output = cmd.arg(arg).output().unwrap().assert();
            output.code(0).stdout(predicates::str::starts_with("mpot "));
        }
    }

    fn help() {
        // --help is successful
        for (arg, starts_with) in [
            ("--help", "GNU Affero version 3 or later"),
            ("-h", "mailing list manager"),
        ] {
            let mut cmd = Command::cargo_bin("mpot").unwrap();
            let output = cmd.arg(arg).output().unwrap().assert();
            output
                .code(0)
                .stdout(predicates::str::starts_with(starts_with))
                .stdout(predicates::str::contains("Usage:"));
        }
    }

    fn sample_config() {
        let mut cmd = Command::cargo_bin("mpot").unwrap();
        // sample-config does not require a configuration file as an argument (but other
        // commands do)
        let output = cmd.arg("sample-config").output().unwrap().assert();
        output.code(0).stdout(predicates::str::is_empty().not());
    }

    fn config_required() {
        let mut cmd = Command::cargo_bin("mpot").unwrap();
        let output = cmd.arg("list-lists").output().unwrap().assert();
        output.code(2).stdout(predicates::str::is_empty()).stderr(
            predicate::eq(
                r#"error: --config is required for mailing list operations

Usage: mpot [OPTIONS] <COMMAND>

For more information, try '--help'."#,
            )
            .trim()
            .normalize(),
        );
    }

    no_args();
    version();
    help();
    sample_config();
    config_required();

    let tmp_dir = TempDir::new().unwrap();

    let conf_path = tmp_dir.path().join("conf.toml");
    let db_path = tmp_dir.path().join("mpot.db");

    let config = Configuration {
        send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
        administrators: vec![],
    };

    let config_str = config.to_toml();

    std::fs::write(&conf_path, config_str.as_bytes()).unwrap();

    fn list_lists(conf: &Path, eq: &str) {
        let mut cmd = Command::cargo_bin("mpot").unwrap();
        let output = cmd
            .arg("-c")
            .arg(conf)
            .arg("list-lists")
            .output()
            .unwrap()
            .assert();
        output
            .code(0)
            .stderr(predicates::str::is_empty())
            .stdout(predicate::eq(eq).trim().normalize());
    }

    list_lists(&conf_path, "No lists found.");

    {
        let db = Connection::open_or_create_db(config).unwrap().trusted();

        let foo_chat = db
            .create_list(MailingList {
                pk: 0,
                name: "foobar chat".into(),
                id: "foo-chat".into(),
                address: "foo-chat@example.com".into(),
                topics: vec![],
                description: None,
                archive_url: None,
            })
            .unwrap();

        assert_eq!(foo_chat.pk(), 1);
    }
    list_lists(
        &conf_path,
        "- foo-chat DbVal(MailingList { pk: 1, name: \"foobar chat\", id: \"foo-chat\", address: \
         \"foo-chat@example.com\", topics: [], description: None, archive_url: None }, 1)\n\tList \
         owners: None\n\tPost policy: None\n\tSubscription policy: None",
    );

    fn create_list(conf: &Path) {
        let mut cmd = Command::cargo_bin("mpot").unwrap();
        let output = cmd
            .arg("-c")
            .arg(conf)
            .arg("create-list")
            .arg("--name")
            .arg("twobar")
            .arg("--id")
            .arg("twobar-chat")
            .arg("--address")
            .arg("twobar-chat@example.com")
            .output()
            .unwrap()
            .assert();
        output.code(0).stderr(predicates::str::is_empty()).stdout(
            predicate::eq("Created new list \"twobar-chat\" with primary key 2")
                .trim()
                .normalize(),
        );
    }
    create_list(&conf_path);
    list_lists(
        &conf_path,
        "- foo-chat DbVal(MailingList { pk: 1, name: \"foobar chat\", id: \"foo-chat\", address: \
         \"foo-chat@example.com\", topics: [], description: None, archive_url: None }, 1)\n\tList \
         owners: None\n\tPost policy: None\n\tSubscription policy: None\n\n- twobar-chat \
         DbVal(MailingList { pk: 2, name: \"twobar\", id: \"twobar-chat\", address: \
         \"twobar-chat@example.com\", topics: [], description: None, archive_url: None }, \
         2)\n\tList owners: None\n\tPost policy: None\n\tSubscription policy: None",
    );
}
