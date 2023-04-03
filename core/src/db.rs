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

fn log_callback(error_code: std::ffi::c_int, message: &str) {
    match error_code {
        rusqlite::ffi::SQLITE_NOTICE => log::info!("{}", message),
        rusqlite::ffi::SQLITE_WARNING => log::warn!("{}", message),
        _ => log::error!("{error_code} {}", message),
    }
}

fn user_authorizer_callback(
    auth_context: rusqlite::hooks::AuthContext<'_>,
) -> rusqlite::hooks::Authorization {
    use rusqlite::hooks::{AuthAction, Authorization};

    match auth_context.action {
        AuthAction::Delete {
            table_name: "error_queue" | "queue" | "candidate_membership" | "membership",
        }
        | AuthAction::Insert {
            table_name: "post" | "error_queue" | "queue" | "candidate_membership" | "membership",
        }
        | AuthAction::Select
        | AuthAction::Savepoint { .. }
        | AuthAction::Transaction { .. }
        | AuthAction::Read { .. }
        | AuthAction::Function {
            function_name: "strftime",
        } => Authorization::Allow,
        _ => Authorization::Deny,
    }
}

impl Database {
    pub fn open_db(conf: Configuration) -> Result<Self> {
        use rusqlite::config::DbConfig;
        use std::sync::Once;

        static INIT_SQLITE_LOGGING: Once = Once::new();

        if !conf.db_path.exists() {
            return Err("Database doesn't exist".into());
        }
        INIT_SQLITE_LOGGING.call_once(|| {
            unsafe { rusqlite::trace::config_log(Some(log_callback)).unwrap() };
        });
        let conn = DbConnection::open(conf.db_path.to_str().unwrap())?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY, true)?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, true)?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)?;
        conn.busy_timeout(core::time::Duration::from_millis(500))?;
        conn.busy_handler(Some(|times: i32| -> bool { times < 5 }))?;
        conn.authorizer(Some(user_authorizer_callback));
        Ok(Database {
            conf,
            connection: conn,
        })
    }

    pub fn trusted(self) -> Self {
        self.connection
            .authorizer::<fn(rusqlite::hooks::AuthContext<'_>) -> rusqlite::hooks::Authorization>(
                None,
            );
        self
    }

    pub fn untrusted(self) -> Self {
        self.connection.authorizer(Some(user_authorizer_callback));
        self
    }

    pub fn open_or_create_db(conf: Configuration) -> Result<Self> {
        if !conf.db_path.exists() {
            let db_path = &conf.db_path;
            use std::os::unix::fs::PermissionsExt;

            info!("Creating database in {}", db_path.display());
            std::fs::File::create(&db_path).context("Could not create db path")?;

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
        Self::open_db(conf)
    }

    pub fn conf(&self) -> &Configuration {
        &self.conf
    }

    pub fn load_archives(&self) -> Result<()> {
        let mut stmt = self.connection.prepare("ATTACH ? AS ?;")?;
        for archive in std::fs::read_dir(&self.conf.data_path)? {
            let archive = archive?;
            let path = archive.path();
            let name = path.file_name().unwrap_or_default();
            if path == self.conf.db_path {
                continue;
            }
            stmt.execute(rusqlite::params![
                path.to_str().unwrap(),
                name.to_str().unwrap()
            ])?;
        }

        Ok(())
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

    /// Remove an existing list policy.
    ///
    /// ```
    /// # use mailpot::{models::*, Configuration, Database, SendMail};
    /// # use tempfile::TempDir;
    ///
    /// # let tmp_dir = TempDir::new().unwrap();
    /// # let db_path = tmp_dir.path().join("mpot.db");
    /// # let config = Configuration {
    /// #     send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
    /// #     db_path: db_path.clone(),
    /// #     storage: "sqlite3".to_string(),
    /// #     data_path: tmp_dir.path().to_path_buf(),
    /// # };
    ///
    /// # fn do_test(config: Configuration) {
    /// let db = Database::open_or_create_db(config).unwrap().trusted();
    /// let list_pk = db.create_list(MailingList {
    ///     pk: 0,
    ///     name: "foobar chat".into(),
    ///     id: "foo-chat".into(),
    ///     address: "foo-chat@example.com".into(),
    ///     description: None,
    ///     archive_url: None,
    /// }).unwrap().pk;
    /// db.set_list_policy(
    ///     PostPolicy {
    ///         pk: 0,
    ///         list: list_pk,
    ///         announce_only: false,
    ///         subscriber_only: true,
    ///         approval_needed: false,
    ///         no_subscriptions: false,
    ///         custom: false,
    ///     },
    /// ).unwrap();
    /// db.remove_list_policy(1, 1).unwrap();
    /// # }
    /// # do_test(config);
    /// ```
    /// ```should_panic
    /// # use mailpot::{models::*, Configuration, Database, SendMail};
    /// # use tempfile::TempDir;
    ///
    /// # let tmp_dir = TempDir::new().unwrap();
    /// # let db_path = tmp_dir.path().join("mpot.db");
    /// # let config = Configuration {
    /// #     send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
    /// #     db_path: db_path.clone(),
    /// #     storage: "sqlite3".to_string(),
    /// #     data_path: tmp_dir.path().to_path_buf(),
    /// # };
    ///
    /// # fn do_test(config: Configuration) {
    /// let db = Database::open_or_create_db(config).unwrap().trusted();
    /// db.remove_list_policy(1, 1).unwrap();
    /// # }
    /// # do_test(config);
    /// ```
    pub fn remove_list_policy(&self, list_pk: i64, policy_pk: i64) -> Result<()> {
        let mut stmt = self
            .connection
            .prepare("DELETE FROM post_policy WHERE pk = ? AND list = ? RETURNING *;")?;
        stmt.query_row(rusqlite::params![&policy_pk, &list_pk,], |_| Ok(()))
            .map_err(|err| {
                if matches!(err, rusqlite::Error::QueryReturnedNoRows) {
                    Error::from(err).chain_err(|| NotFound("list or list policy not found!"))
                } else {
                    err.into()
                }
            })?;

        trace!("remove_list_policy {} {}.", list_pk, policy_pk);
        Ok(())
    }

    pub fn set_list_policy(&self, policy: PostPolicy) -> Result<DbVal<PostPolicy>> {
        if !(policy.announce_only
            || policy.subscriber_only
            || policy.approval_needed
            || policy.no_subscriptions
            || policy.custom)
        {
            return Err(
                "Cannot add empty policy. Having no policies is probably what you want to do."
                    .into(),
            );
        }
        let list_pk = policy.list;

        let mut stmt = self.connection.prepare("INSERT OR REPLACE INTO post_policy(list, announce_only, subscriber_only, approval_needed, no_subscriptions, custom) VALUES (?, ?, ?, ?, ?, ?) RETURNING *;")?;
        let ret = stmt
            .query_row(
                rusqlite::params![
                    &list_pk,
                    &policy.announce_only,
                    &policy.subscriber_only,
                    &policy.approval_needed,
                    &policy.no_subscriptions,
                    &policy.custom,
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
                            no_subscriptions: row.get("no_subscriptions")?,
                            custom: row.get("custom")?,
                        },
                        pk,
                    ))
                },
            )
            .map_err(|err| {
                if matches!(
                    err,
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error {
                            code: rusqlite::ffi::ErrorCode::ConstraintViolation,
                            extended_code: 787
                        },
                        _
                    )
                ) {
                    Error::from(err).chain_err(|| NotFound("Could not find a list with this pk."))
                } else {
                    err.into()
                }
            })?;

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
            .prepare("SELECT pk, list, address, message_id, message, timestamp, datetime, strftime('%Y-%m', CAST(timestamp AS INTEGER), 'unixepoch') as month_year FROM post WHERE list = ?;")?;
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
                    month_year: row.get("month_year")?,
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
                        no_subscriptions: row.get("no_subscriptions")?,
                        custom: row.get("custom")?,
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
            .query_row(
                "DELETE FROM list_owner WHERE list = ? AND pk = ? RETURNING *;",
                rusqlite::params![&list_pk, &owner_pk],
                |_| Ok(()),
            )
            .map_err(|err| {
                if matches!(err, rusqlite::Error::QueryReturnedNoRows) {
                    Error::from(err).chain_err(|| NotFound("list or list owner not found!"))
                } else {
                    err.into()
                }
            })?;
        Ok(())
    }

    pub fn add_list_owner(&self, list_owner: ListOwner) -> Result<DbVal<ListOwner>> {
        let mut stmt = self.connection.prepare(
            "INSERT OR REPLACE INTO list_owner(list, address, name) VALUES (?, ?, ?) RETURNING *;",
        )?;
        let list_pk = list_owner.list;
        let ret = stmt
            .query_row(
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
            )
            .map_err(|err| {
                if matches!(
                    err,
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error {
                            code: rusqlite::ffi::ErrorCode::ConstraintViolation,
                            extended_code: 787
                        },
                        _
                    )
                ) {
                    Error::from(err).chain_err(|| NotFound("Could not find a list with this pk."))
                } else {
                    err.into()
                }
            })?;

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
