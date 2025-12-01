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

use std::{fs::File, io::Write};

use mailpot::{Configuration, Connection, SendMail};
use mailpot_tests::init_stderr_logging;
use tempfile::TempDir;

include!("../build/make_migrations.rs");

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

    let db = Connection::open_or_create_db(config).unwrap().trusted();

    let migrations = Connection::MIGRATIONS;
    if migrations.is_empty() {
        return;
    }

    let version = db.schema_version().unwrap();

    assert_eq!(version, migrations[migrations.len() - 1].0);

    db.migrate(version, migrations[0].0).unwrap();

    db.migrate(migrations[0].0, version).unwrap();
}

trait ConnectionExt {
    fn schema_version(&self) -> Result<u32, rusqlite::Error>;
    fn migrate(
        &mut self,
        from: u32,
        to: u32,
        migrations: &[(u32, &str, &str)],
    ) -> Result<(), rusqlite::Error>;
}

impl ConnectionExt for rusqlite::Connection {
    fn schema_version(&self) -> Result<u32, rusqlite::Error> {
        self.prepare("SELECT user_version FROM pragma_user_version;")?
            .query_row([], |row| {
                let v: u32 = row.get(0)?;
                Ok(v)
            })
    }

    fn migrate(
        &mut self,
        mut from: u32,
        to: u32,
        migrations: &[(u32, &str, &str)],
    ) -> Result<(), rusqlite::Error> {
        if from == to {
            return Ok(());
        }

        let undo = from > to;
        let tx = self.transaction()?;

        loop {
            log::trace!(
                "exec migration from {from} to {to}, type: {}do",
                if undo { "un" } else { "re" }
            );
            if undo {
                log::trace!("{}", migrations[from as usize - 1].2);
                tx.execute_batch(migrations[from as usize - 1].2)?;
                from -= 1;
                if from == to {
                    break;
                }
            } else {
                if from != 0 {
                    log::trace!("{}", migrations[from as usize - 1].1);
                    tx.execute_batch(migrations[from as usize - 1].1)?;
                }
                from += 1;
                if from == to + 1 {
                    break;
                }
            }
        }
        tx.pragma_update(
            None,
            "user_version",
            if to == 0 {
                0
            } else {
                migrations[to as usize - 1].0
            },
        )?;

        tx.commit()?;
        Ok(())
    }
}

const FIRST_SCHEMA: &str = r#"
PRAGMA foreign_keys = true;
PRAGMA encoding = 'UTF-8';
PRAGMA schema_version = 0;

CREATE TABLE IF NOT EXISTS person (
  pk               INTEGER PRIMARY KEY NOT NULL,
  name             TEXT,
  address          TEXT NOT NULL,
  created          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified    INTEGER NOT NULL DEFAULT (unixepoch())
);
"#;

const MIGRATIONS: &[(u32, &str, &str)] = &[
    (
        1,
        "ALTER TABLE PERSON ADD COLUMN interests TEXT;",
        "ALTER TABLE PERSON DROP COLUMN interests;",
    ),
    (
        2,
        "CREATE TABLE hobby ( pk INTEGER PRIMARY KEY NOT NULL,title TEXT NOT NULL);",
        "DROP TABLE hobby;",
    ),
    (
        3,
        "ALTER TABLE PERSON ADD COLUMN main_hobby INTEGER REFERENCES hobby(pk) ON DELETE SET NULL;",
        "ALTER TABLE PERSON DROP COLUMN main_hobby;",
    ),
];

#[test]
fn test_migration_gen() {
    init_stderr_logging();
    let tmp_dir = TempDir::new().unwrap();
    let in_path = tmp_dir.path().join("migrations");
    std::fs::create_dir(&in_path).unwrap();
    let out_path = tmp_dir.path().join("migrations.txt");
    for (num, redo, undo) in MIGRATIONS.iter() {
        let mut redo_file = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(in_path.join(format!("{num:03}.sql")))
            .unwrap();
        redo_file.write_all(redo.as_bytes()).unwrap();
        redo_file.flush().unwrap();

        let mut undo_file = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(in_path.join(format!("{num:03}.undo.sql")))
            .unwrap();
        undo_file.write_all(undo.as_bytes()).unwrap();
        undo_file.flush().unwrap();
    }

    make_migrations(&in_path, &out_path, &mut vec![]);
    let output = std::fs::read_to_string(&out_path).unwrap();
    assert_eq!(&output.replace([' ', '\n'], ""), &r###"//(user_version, redo sql, undo sql
&[(1,r##"ALTER TABLE PERSON ADD COLUMN interests TEXT;"##,r##"ALTER TABLE PERSON DROP COLUMN interests;"##),(2,r##"CREATE TABLE hobby ( pk INTEGER PRIMARY KEY NOT NULL,title TEXT NOT NULL);"##,r##"DROP TABLE hobby;"##),(3,r##"ALTER TABLE PERSON ADD COLUMN main_hobby INTEGER REFERENCES hobby(pk) ON DELETE SET NULL;"##,r##"ALTER TABLE PERSON DROP COLUMN main_hobby;"##),]"###.replace([' ', '\n'], ""));
}

#[test]
#[should_panic]
fn test_migration_gen_panic() {
    init_stderr_logging();
    let tmp_dir = TempDir::new().unwrap();
    let in_path = tmp_dir.path().join("migrations");
    std::fs::create_dir(&in_path).unwrap();
    let out_path = tmp_dir.path().join("migrations.txt");
    for (num, redo, undo) in MIGRATIONS.iter().skip(1) {
        let mut redo_file = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(in_path.join(format!("{num:03}.sql")))
            .unwrap();
        redo_file.write_all(redo.as_bytes()).unwrap();
        redo_file.flush().unwrap();

        let mut undo_file = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(in_path.join(format!("{num:03}.undo.sql")))
            .unwrap();
        undo_file.write_all(undo.as_bytes()).unwrap();
        undo_file.flush().unwrap();
    }

    make_migrations(&in_path, &out_path, &mut vec![]);
    let output = std::fs::read_to_string(&out_path).unwrap();
    assert_eq!(&output.replace([' ','\n'], ""), &r#"//(user_version, redo sql, undo sql
&[(1,"ALTER TABLE PERSON ADD COLUMN interests TEXT;","ALTER TABLE PERSON DROP COLUMN interests;"),(2,"CREATE TABLE hobby ( pk INTEGER PRIMARY KEY NOT NULL,title TEXT NOT NULL);","DROP TABLE hobby;"),(3,"ALTER TABLE PERSON ADD COLUMN main_hobby INTEGER REFERENCES hobby(pk) ON DELETE SET NULL;","ALTER TABLE PERSON DROP COLUMN main_hobby;"),]"#.replace([' ', '\n'], ""));
}

#[test]
fn test_migration() {
    init_stderr_logging();
    let tmp_dir = TempDir::new().unwrap();
    let db_path = tmp_dir.path().join("migr.db");

    let mut conn = rusqlite::Connection::open(db_path.to_str().unwrap()).unwrap();
    conn.execute_batch(FIRST_SCHEMA).unwrap();

    conn.execute_batch(
        "INSERT INTO person(name,address) VALUES('John Doe', 'johndoe@example.com');",
    )
    .unwrap();

    let version = conn.schema_version().unwrap();
    log::trace!("initial schema version is {}", version);

    //assert_eq!(version, migrations[migrations.len() - 1].0);

    conn.migrate(version, MIGRATIONS.last().unwrap().0, MIGRATIONS)
        .unwrap();
    /*
     * CREATE TABLE sqlite_schema (
     type text,
     name text,
     tbl_name text,
     rootpage integer,
     sql text
     );
    */
    let get_sql = |table: &str, conn: &rusqlite::Connection| -> String {
        conn.prepare("SELECT sql FROM sqlite_schema WHERE name = ?;")
            .unwrap()
            .query_row([table], |row| {
                let sql: String = row.get(0)?;
                Ok(sql)
            })
            .unwrap()
    };

    let strip_ws = |sql: &str| -> String { sql.replace([' ', '\n'], "") };

    let person_sql: String = get_sql("person", &conn);
    assert_eq!(
        &strip_ws(&person_sql),
        &strip_ws(
            r#"
CREATE TABLE person (
  pk               INTEGER PRIMARY KEY NOT NULL,
  name             TEXT,
  address          TEXT NOT NULL,
  created          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified    INTEGER NOT NULL DEFAULT (unixepoch()),
  interests TEXT,
  main_hobby INTEGER REFERENCES hobby(pk) ON DELETE SET NULL
)"#
        )
    );
    let hobby_sql: String = get_sql("hobby", &conn);
    assert_eq!(
        &strip_ws(&hobby_sql),
        &strip_ws(
            r#"CREATE TABLE hobby (
        pk INTEGER PRIMARY KEY NOT NULL,
        title TEXT NOT NULL
)"#
        )
    );
    conn.execute_batch(
        r#"
        INSERT INTO hobby(title) VALUES('fishing');
        INSERT INTO hobby(title) VALUES('reading books');
        INSERT INTO hobby(title) VALUES('running');
        INSERT INTO hobby(title) VALUES('forest walks');
        UPDATE person SET main_hobby = hpk FROM (SELECT pk AS hpk FROM hobby LIMIT 1) WHERE name = 'John Doe';
        "#
    )
    .unwrap();
    log::trace!(
        "John Doe's main hobby is {:?}",
        conn.prepare(
            "SELECT pk, title FROM hobby WHERE EXISTS (SELECT 1 FROM person AS p WHERE \
             p.main_hobby = pk);"
        )
        .unwrap()
        .query_row([], |row| {
            let pk: i64 = row.get(0)?;
            let title: String = row.get(1)?;
            Ok((pk, title))
        })
        .unwrap()
    );

    conn.migrate(MIGRATIONS.last().unwrap().0, 0, MIGRATIONS)
        .unwrap();

    assert_eq!(
        conn.prepare("SELECT sql FROM sqlite_schema WHERE name = 'hobby';")
            .unwrap()
            .query_row([], |row| { row.get::<_, String>(0) })
            .unwrap_err(),
        rusqlite::Error::QueryReturnedNoRows
    );
    let person_sql: String = get_sql("person", &conn);
    assert_eq!(
        &strip_ws(&person_sql),
        &strip_ws(
            r#"
CREATE TABLE person (
  pk               INTEGER PRIMARY KEY NOT NULL,
  name             TEXT,
  address          TEXT NOT NULL,
  created          INTEGER NOT NULL DEFAULT (unixepoch()),
  last_modified    INTEGER NOT NULL DEFAULT (unixepoch())
)"#
        )
    );
}
