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

#![deny(dead_code)]

use std::path::Path;

use assert_cmd::{assert::OutputAssertExt, cargo, Command};
use mailpot::{models::*, Configuration, Connection, SendMail};
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_cli_basic_interfaces() {
    fn no_args() {
        // 2 -> incorrect usage
        Command::new(cargo::cargo_bin!("mpot")).assert().code(2);
    }

    fn version() {
        // --version is successful
        for arg in ["--version", "-V"] {
            let output = Command::new(cargo::cargo_bin!("mpot"))
                .arg(arg)
                .output()
                .unwrap()
                .assert();
            output.code(0).stdout(predicates::str::starts_with("mpot "));
        }
    }

    fn help() {
        // --help is successful
        for (arg, starts_with) in [
            ("--help", "GNU Affero version 3 or later"),
            ("-h", "mailing list manager"),
        ] {
            let output = Command::new(cargo::cargo_bin!("mpot"))
                .arg(arg)
                .output()
                .unwrap()
                .assert();
            output
                .code(0)
                .stdout(predicates::str::starts_with(starts_with))
                .stdout(predicates::str::contains("Usage:"));
        }
    }

    fn sample_config() {
        // sample-config does not require a configuration file as an argument (but other
        // commands do)
        let output = Command::new(cargo::cargo_bin!("mpot"))
            .arg("sample-config")
            .output()
            .unwrap()
            .assert();
        output.code(0).stdout(predicates::str::is_empty().not());
    }

    fn config_required() {
        let output = Command::new(cargo::cargo_bin!("mpot"))
            .arg("list-lists")
            .output()
            .unwrap()
            .assert();
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

    fn config_not_exists(conf: &Path) {
        let output = Command::new(cargo::cargo_bin!("mpot"))
            .arg("-c")
            .arg(conf)
            .arg("list-lists")
            .output()
            .unwrap()
            .assert();
        output.code(255).stderr(predicates::str::is_empty()).stdout(
            predicate::eq(
                format!(
                    "[1] Could not read configuration file from path: {path} Caused by:\n[2] \
                     Configuration file {path} not found. Caused by:\n[3] Error returned from \
                     internal I/O operation: No such file or directory (os error 2)",
                    path = conf.display()
                )
                .as_str(),
            )
            .trim()
            .normalize(),
        );
    }

    config_not_exists(&conf_path);

    std::fs::write(&conf_path, config_str.as_bytes()).unwrap();

    fn list_lists(conf: &Path, eq: &str) {
        let output = Command::new(cargo::cargo_bin!("mpot"))
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
        let output = Command::new(cargo::cargo_bin!("mpot"))
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

    fn add_list_owner(conf: &Path) {
        let output = Command::new(cargo::cargo_bin!("mpot"))
            .arg("-c")
            .arg(conf)
            .arg("list")
            .arg("twobar-chat")
            .arg("add-list-owner")
            .arg("--address")
            .arg("list-owner@example.com")
            .output()
            .unwrap()
            .assert();
        output.code(0).stderr(predicates::str::is_empty()).stdout(
            predicate::eq("Added new list owner [#1 2] list-owner@example.com")
                .trim()
                .normalize(),
        );
    }
    add_list_owner(&conf_path);
    list_lists(
        &conf_path,
        "- foo-chat DbVal(MailingList { pk: 1, name: \"foobar chat\", id: \"foo-chat\", address: \
         \"foo-chat@example.com\", topics: [], description: None, archive_url: None }, 1)\n\tList \
         owners: None\n\tPost policy: None\n\tSubscription policy: None\n\n- twobar-chat \
         DbVal(MailingList { pk: 2, name: \"twobar\", id: \"twobar-chat\", address: \
         \"twobar-chat@example.com\", topics: [], description: None, archive_url: None }, \
         2)\n\tList owners:\n\t- [#1 2] list-owner@example.com\n\tPost policy: \
         None\n\tSubscription policy: None",
    );

    fn remove_list_owner(conf: &Path) {
        let output = Command::new(cargo::cargo_bin!("mpot"))
            .arg("-c")
            .arg(conf)
            .arg("list")
            .arg("twobar-chat")
            .arg("remove-list-owner")
            .arg("--pk")
            .arg("1")
            .output()
            .unwrap()
            .assert();
        output.code(0).stderr(predicates::str::is_empty()).stdout(
            predicate::eq("Removed list owner with pk = 1")
                .trim()
                .normalize(),
        );
    }
    remove_list_owner(&conf_path);
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
