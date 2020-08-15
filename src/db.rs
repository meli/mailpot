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

use super::*;
use melib::Envelope;
pub use rusqlite::{self, params, Connection};
use std::convert::TryFrom;
use std::path::PathBuf;

const DB_NAME: &str = "mpot.db";
const INIT_SCRIPT: &str = "PRAGMA foreign_keys = true;
    PRAGMA encoding = 'UTF-8';

    CREATE TABLE IF NOT EXISTS mailing_lists (
                    pk              INTEGER PRIMARY KEY,
                    name            TEXT NOT NULL,
                    id              TEXT NOT NULL,
                    address         TEXT NOT NULL,
                    archive_url     TEXT,
                    description     TEXT
                   );
    CREATE TABLE IF NOT EXISTS membership (
                list              INTEGER NOT NULL,
                address          TEXT NOT NULL,
                name          TEXT,
                digest          BOOLEAN NOT NULL DEFAULT 0,
                hide_address    BOOLEAN NOT NULL DEFAULT 0,
                receive_duplicates    BOOLEAN NOT NULL DEFAULT 1,
                receive_own_posts    BOOLEAN NOT NULL DEFAULT 0,
                receive_confirmation    BOOLEAN NOT NULL DEFAULT 1,
                PRIMARY KEY (list, address),
                FOREIGN KEY (list) REFERENCES mailing_lists ON DELETE CASCADE
               );
    CREATE INDEX IF NOT EXISTS mailing_lists_idx ON mailing_lists(id);
    CREATE INDEX IF NOT EXISTS membership_idx ON membership(address);";

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn list_lists(&self) -> Result<Vec<MailingList>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM mailing_lists;")
            .unwrap();
        let res = stmt
            .query_map(params![], |row| MailingList::try_from(row))?
            .map(|r| r.map_err(|err| err.into()))
            .collect();
        res
    }

    pub fn list_members(&self, pk: i64) -> Result<Vec<ListMembership>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM membership WHERE list = ?;")
            .unwrap();
        let res = stmt
            .query_map(params![pk], |row| ListMembership::try_from(row))?
            .map(|r| r.map_err(|err| err.into()))
            .collect();
        res
    }

    pub fn add_member(&self, list_pk: i64, new_val: ListMembership) -> Result<()> {
        self.connection.execute(
            "INSERT INTO membership (list, address, name, digest, hide_address) VALUES (?1, ?2, ?3, ?4, ?5);",
            params![
                &list_pk,
                &new_val.address,
                &new_val.name,
                &new_val.digest,
                &new_val.hide_address
            ],
        )?;
        Ok(())
    }

    pub fn remove_member(&self, list_pk: i64, address: &str) -> Result<()> {
        if self.connection.execute(
            "DELETE FROM membership WHERE list = ?1 AND address = ?2;",
            params![&list_pk, &address,],
        )? == 0
        {
            Err(format!("Address {} is not a member of this list.", address))?;
        }
        Ok(())
    }

    pub fn create_list(&self, new_val: MailingList) -> Result<()> {
        self.connection.execute(
            "INSERT INTO mailing_lists (name, id, address, description) VALUES (?1, ?2, ?3, ?4)",
            params![
                &new_val.name,
                &new_val.id,
                &new_val.address,
                &new_val.description
            ],
        )?;
        Ok(())
    }

    pub fn db_path() -> Result<PathBuf> {
        let name = DB_NAME;
        let data_dir = xdg::BaseDirectories::with_prefix("mailpot")?;
        Ok(data_dir.place_data_file(name)?)
    }

    pub fn open_db(db_path: PathBuf) -> Result<Self> {
        if !db_path.exists() {
            return Err("Database doesn't exist".into());
        }
        Ok(Database {
            connection: Connection::open(&db_path)?,
        })
    }

    pub fn open_or_create_db() -> Result<Self> {
        let db_path = Self::db_path()?;
        let mut set_mode = false;
        if !db_path.exists() {
            println!("Creating {} database in {}", DB_NAME, db_path.display());
            set_mode = true;
        }
        let conn = Connection::open(&db_path)?;
        if set_mode {
            use std::os::unix::fs::PermissionsExt;
            let file = std::fs::File::open(&db_path)?;
            let metadata = file.metadata()?;
            let mut permissions = metadata.permissions();

            permissions.set_mode(0o600); // Read/write for owner only.
            file.set_permissions(permissions)?;
        }

        conn.execute_batch(INIT_SCRIPT)?;

        Ok(Database { connection: conn })
    }

    pub fn get_list_filters(&self, _list: &MailingList) -> Vec<Box<dyn crate::post::PostFilter>> {
        use crate::post::*;
        vec![
            Box::new(FixCRLF),
            Box::new(PostRightsCheck),
            Box::new(AddListHeaders),
            Box::new(FinalizeRecipients),
        ]
    }

    pub fn post(&self, env: Envelope, raw: &[u8]) -> Result<()> {
        let mut lists = self.list_lists()?;
        let tos = env
            .to()
            .iter()
            .map(|addr| addr.get_email())
            .collect::<Vec<String>>();
        if tos.is_empty() {
            return Err("Envelope To: field is empty!".into());
        }
        if env.from().is_empty() {
            return Err("Envelope From: field is empty!".into());
        }

        lists.retain(|list| tos.iter().any(|a| a == &list.address));
        if lists.is_empty() {
            return Err("Envelope To: field doesn't contain any known lists!".into());
        }

        use crate::post::{Post, PostAction};
        for mut list in lists {
            let filters = self.get_list_filters(&list);
            let memberships = self.list_members(list.pk)?;
            let mut post = Post {
                list: &mut list,
                from: env.from()[0].clone(),
                memberships: &memberships,
                bytes: raw.to_vec(),
                to: env.to().to_vec(),
                action: PostAction::Defer {
                    reason: "Default action.".into(),
                },
            };
            let result = {
                let result: std::result::Result<_, String> = filters
                    .into_iter()
                    .fold(Ok(&mut post), |post, f| post.and_then(|p| f.feed(p)));
                format!("{:#?}", result)
            };
            eprintln!("result for list {} is {}", list, result);
        }
        /*

        use melib::futures;
        use melib::smol;
        use melib::smtp::*;
        std::thread::spawn(|| smol::run(futures::future::pending::<()>()));
        let mut conn = futures::executor::block_on(SmtpConnection::new_connection(conf)).unwrap();
        futures::executor::block_on(conn.mail_transaction(raw, )).unwrap();
               */
        Ok(())
    }
}
