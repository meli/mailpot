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

use mailpot::{Configuration, Connection, SendMail};
use mailpot_tests::init_stderr_logging;
use tempfile::TempDir;

#[test]
fn test_init_empty() {
    init_stderr_logging();
    let tmp_dir = TempDir::new().unwrap();

    let db_path = tmp_dir.path().join("mpot.db");
    let config = Configuration {
        send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
        administrators: vec![],
    };

    let mut db = Connection::open_or_create_db(config).unwrap().trusted();

    let migrations = Connection::MIGRATIONS;
    if migrations.is_empty() {
        return;
    }

    let version = db.schema_version().unwrap();

    assert_eq!(version, migrations[migrations.len() - 1].0);

    db.migrate(version, migrations[0].0).unwrap();

    db.migrate(migrations[0].0, version).unwrap();
}
