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

use super::Configuration;
use super::*;
use crate::ErrorKind::*;
use melib::Envelope;
use models::changesets::*;
use rusqlite::Connection as DbConnection;
use rusqlite::OptionalExtension;
use std::convert::TryFrom;
use std::io::Write;
use std::process::{Command, Stdio};

const DB_NAME: &str = "current.db";

pub struct Database {
    pub connection: DbConnection,
    conf: Configuration,
}

mod error_queue;
pub use error_queue::*;
mod posts;
pub use posts::*;
mod members;
pub use members::*;

impl Database {
    pub fn open_db(conf: &Configuration) -> Result<Self> {
        if !conf.db_path.exists() {
            return Err("Database doesn't exist".into());
        }
        Ok(Database {
            conf: conf.clone(),
            connection: DbConnection::open(conf.db_path.to_str().unwrap())?,
        })
    }

    pub fn open_or_create_db(conf: &Configuration) -> Result<Self> {
        let mut db_path = conf.db_path.to_path_buf();
        if db_path.is_dir() {
            db_path.push(DB_NAME);
        }
        let mut create = false;
        if !db_path.exists() {
            info!("Creating {} database in {}", DB_NAME, db_path.display());
            create = true;
            std::fs::File::create(&db_path).context("Could not create db path")?;
        }
        if create {
            use std::os::unix::fs::PermissionsExt;
            let mut child = Command::new("sqlite3")
                .arg(&db_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            let mut stdin = child.stdin.take().unwrap();
            std::thread::spawn(move || {
                stdin
                    .write_all(include_bytes!("./schema.sql"))
                    .expect("failed to write to stdin");
                stdin.flush().expect("could not flush stdin");
            });
            let output = child.wait_with_output()?;
            if !output.status.success() {
                return Err(format!("Could not initialize sqlite3 database at {}: sqlite3 returned exit code {} and stderr {} {}", db_path.display(), output.status.code().unwrap_or_default(), String::from_utf8_lossy(&output.stderr), String::from_utf8_lossy(&output.stdout)).into());
            }

            let file = std::fs::File::open(&db_path)?;
            let metadata = file.metadata()?;
            let mut permissions = metadata.permissions();

            permissions.set_mode(0o600); // Read/write for owner only.
            file.set_permissions(permissions)?;
        }
        db_path = db_path
            .canonicalize()
            .context("Could not canonicalize db path")?;

        let conn = DbConnection::open(db_path.to_str().unwrap())?;

        Ok(Database {
            conf: conf.clone(),
            connection: conn,
        })
    }

    pub fn load_archives(&mut self, conf: &Configuration) -> Result<&mut Self> {
        let archives_path = conf.data_path.clone();
        let mut stmt = self.connection.prepare("ATTACH ? AS ?;")?;
        for archive in std::fs::read_dir(&archives_path)? {
            let archive = archive?;
            let path = archive.path();
            let name = path.file_name().unwrap_or_default();
            if name == DB_NAME {
                continue;
            }
            stmt.execute(rusqlite::params![
                path.to_str().unwrap(),
                name.to_str().unwrap()
            ])?;
        }
        drop(stmt);

        Ok(self)
    }

    pub fn list_lists(&self) -> Result<Vec<DbVal<MailingList>>> {
        let mut stmt = self.connection.prepare("SELECT * FROM mailing_lists;")?;
        let list_iter = stmt.query_map([], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                MailingList {
                    pk,
                    name: row.get("name")?,
                    id: row.get("id")?,
                    address: row.get("address")?,
                    description: row.get("description")?,
                    archive_url: row.get("archive_url")?,
                },
                pk,
            ))
        })?;

        let mut ret = vec![];
        for list in list_iter {
            let list = list?;
            ret.push(list);
        }
        Ok(ret)
    }

    pub fn get_list(&self, pk: i64) -> Result<Option<DbVal<MailingList>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM mailing_lists WHERE pk = ?;")?;
        let ret = stmt
            .query_row([&pk], |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    MailingList {
                        pk,
                        name: row.get("name")?,
                        id: row.get("id")?,
                        address: row.get("address")?,
                        description: row.get("description")?,
                        archive_url: row.get("archive_url")?,
                    },
                    pk,
                ))
            })
            .optional()?;

        Ok(ret)
    }

    pub fn get_list_by_id<S: AsRef<str>>(&self, id: S) -> Result<Option<DbVal<MailingList>>> {
        let id = id.as_ref();
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM mailing_lists WHERE id = ?;")?;
        let ret = stmt
            .query_row([&id], |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    MailingList {
                        pk,
                        name: row.get("name")?,
                        id: row.get("id")?,
                        address: row.get("address")?,
                        description: row.get("description")?,
                        archive_url: row.get("archive_url")?,
                    },
                    pk,
                ))
            })
            .optional()?;

        Ok(ret)
    }

    pub fn create_list(&self, new_val: MailingList) -> Result<DbVal<MailingList>> {
        let mut stmt = self
            .connection
            .prepare("INSERT INTO mailing_lists(name, id, address, description, archive_url) VALUES(?, ?, ?, ?, ?) RETURNING *;")?;
        let ret = stmt.query_row(
            rusqlite::params![
                &new_val.name,
                &new_val.id,
                &new_val.address,
                new_val.description.as_ref(),
                new_val.archive_url.as_ref(),
            ],
            |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    MailingList {
                        pk,
                        name: row.get("name")?,
                        id: row.get("id")?,
                        address: row.get("address")?,
                        description: row.get("description")?,
                        archive_url: row.get("archive_url")?,
                    },
                    pk,
                ))
            },
        )?;

        trace!("create_list {:?}.", &ret);
        Ok(ret)
    }

    pub fn remove_list_policy(&self, list_pk: i64, policy_pk: i64) -> Result<()> {
        let mut stmt = self
            .connection
            .prepare("DELETE FROM post_policy WHERE pk = ? AND list = ?;")?;
        stmt.execute(rusqlite::params![&policy_pk, &list_pk,])?;

        trace!("remove_list_policy {} {}.", list_pk, policy_pk);
        Ok(())
    }

    pub fn set_list_policy(&self, list_pk: i64, policy: PostPolicy) -> Result<DbVal<PostPolicy>> {
        if !(policy.announce_only || policy.subscriber_only || policy.approval_needed) {
            return Err(
                "Cannot add empty policy. Having no policies is probably what you want to do."
                    .into(),
            );
        }

        let mut stmt = self.connection.prepare("INSERT OR REPLACE INTO post_policy(list, announce_only, subscriber_only, approval_needed) VALUES (?, ?, ?, ?) RETURNING *;")?;
        let ret = stmt.query_row(
            rusqlite::params![
                &list_pk,
                &policy.announce_only,
                &policy.subscriber_only,
                &policy.approval_needed,
            ],
            |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    PostPolicy {
                        pk,
                        list: row.get("list")?,
                        announce_only: row.get("announce_only")?,
                        subscriber_only: row.get("subscriber_only")?,
                        approval_needed: row.get("approval_needed")?,
                    },
                    pk,
                ))
            },
        )?;

        trace!("set_list_policy {:?}.", &ret);
        Ok(ret)
    }

    pub fn list_posts(
        &self,
        list_pk: i64,
        _date_range: Option<(String, String)>,
    ) -> Result<Vec<DbVal<Post>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM post WHERE list = ?;")?;
        let iter = stmt.query_map(rusqlite::params![&list_pk,], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                Post {
                    pk,
                    list: row.get("list")?,
                    address: row.get("address")?,
                    message_id: row.get("message_id")?,
                    message: row.get("message")?,
                    timestamp: row.get("timestamp")?,
                    datetime: row.get("datetime")?,
                },
                pk,
            ))
        })?;
        let mut ret = vec![];
        for post in iter {
            let post = post?;
            ret.push(post);
        }

        trace!("list_posts {:?}.", &ret);
        Ok(ret)
    }

    pub fn update_list(&self, _change_set: MailingListChangeset) -> Result<()> {
        /*
        diesel::update(mailing_lists::table)
            .set(&set)
            .execute(&self.connection)?;
        */
        Ok(())
    }

    pub fn get_list_policy(&self, pk: i64) -> Result<Option<DbVal<PostPolicy>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM post_policy WHERE list = ?;")?;
        let ret = stmt
            .query_row([&pk], |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    PostPolicy {
                        pk,
                        list: row.get("list")?,
                        announce_only: row.get("announce_only")?,
                        subscriber_only: row.get("subscriber_only")?,
                        approval_needed: row.get("approval_needed")?,
                    },
                    pk,
                ))
            })
            .optional()?;

        Ok(ret)
    }

    pub fn get_list_owners(&self, pk: i64) -> Result<Vec<DbVal<ListOwner>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM list_owner WHERE list = ?;")?;
        let list_iter = stmt.query_map([&pk], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                ListOwner {
                    pk,
                    list: row.get("list")?,
                    address: row.get("address")?,
                    name: row.get("name")?,
                },
                pk,
            ))
        })?;

        let mut ret = vec![];
        for list in list_iter {
            let list = list?;
            ret.push(list);
        }
        Ok(ret)
    }

    pub fn remove_list_owner(&self, list_pk: i64, owner_pk: i64) -> Result<()> {
        self.connection
            .execute(
                "DELETE FROM list_owners WHERE list_pk = ? AND pk = ?;",
                rusqlite::params![&list_pk, &owner_pk],
            )
            .chain_err(|| NotFound("List owner"))?;
        Ok(())
    }

    pub fn add_list_owner(&self, list_pk: i64, list_owner: ListOwner) -> Result<DbVal<ListOwner>> {
        let mut stmt = self.connection.prepare(
            "INSERT OR REPLACE INTO list_owner(list, address, name) VALUES (?, ?, ?) RETURNING *;",
        )?;
        let ret = stmt.query_row(
            rusqlite::params![&list_pk, &list_owner.address, &list_owner.name,],
            |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    ListOwner {
                        pk,
                        list: row.get("list")?,
                        address: row.get("address")?,
                        name: row.get("name")?,
                    },
                    pk,
                ))
            },
        )?;

        trace!("add_list_owner {:?}.", &ret);
        Ok(ret)
    }

    pub fn get_list_filters(
        &self,
        _list: &DbVal<MailingList>,
    ) -> Vec<Box<dyn crate::mail::message_filters::PostFilter>> {
        use crate::mail::message_filters::*;
        vec![
            Box::new(FixCRLF),
            Box::new(PostRightsCheck),
            Box::new(AddListHeaders),
            Box::new(FinalizeRecipients),
        ]
    }
}
