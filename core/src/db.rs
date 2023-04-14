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

//! Mailpot database and methods.

use super::Configuration;
use super::*;
use crate::ErrorKind::*;
use melib::Envelope;
use models::changesets::*;
use rusqlite::Connection as DbConnection;
use rusqlite::OptionalExtension;
use std::io::Write;
use std::process::{Command, Stdio};

/// A connection to a `mailpot` database.
pub struct Connection {
    /// The `rusqlite` connection handle.
    pub connection: DbConnection,
    conf: Configuration,
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Connection")
            .field("conf", &self.conf)
            .finish()
    }
}

mod error_queue;
pub use error_queue::*;
mod posts;
pub use posts::*;
mod subscriptions;
pub use subscriptions::*;
mod policies;
pub use policies::*;

fn log_callback(error_code: std::ffi::c_int, message: &str) {
    match error_code {
        rusqlite::ffi::SQLITE_NOTICE => log::info!("{}", message),
        rusqlite::ffi::SQLITE_WARNING => log::warn!("{}", message),
        _ => log::error!("{error_code} {}", message),
    }
}
// INSERT INTO subscription(list, address, name, enabled, digest, verified, hide_address, receive_duplicates, receive_own_posts, receive_confirmation) VALUES
fn user_authorizer_callback(
    auth_context: rusqlite::hooks::AuthContext<'_>,
) -> rusqlite::hooks::Authorization {
    use rusqlite::hooks::{AuthAction, Authorization};

    // [ref:sync_auth_doc] sync with `untrusted()` rustdoc when changing this.
    match auth_context.action {
        AuthAction::Delete {
            table_name: "queue" | "candidate_subscription" | "subscription",
        }
        | AuthAction::Insert {
            table_name: "post" | "queue" | "candidate_subscription" | "subscription" | "account",
        }
        | AuthAction::Update {
            table_name: "candidate_subscription" | "templates",
            column_name: "accepted" | "last_modified" | "verified" | "address",
        }
        | AuthAction::Update {
            table_name: "account",
            column_name: "last_modified" | "name" | "public_key" | "password",
        }
        | AuthAction::Update {
            table_name: "subscription",
            column_name:
                "last_modified"
                | "account"
                | "digest"
                | "verified"
                | "hide_address"
                | "receive_duplicates"
                | "receive_own_posts"
                | "receive_confirmation",
        }
        | AuthAction::Select
        | AuthAction::Savepoint { .. }
        | AuthAction::Transaction { .. }
        | AuthAction::Read { .. }
        | AuthAction::Function {
            function_name: "strftime" | "unixepoch" | "datetime",
        } => Authorization::Allow,
        _ => Authorization::Deny,
    }
}

impl Connection {
    /// Creates a new database connection.
    ///
    /// `Connection` supports a limited subset of operations by default (see
    /// [`Connection::untrusted`]).
    /// Use [`Connection::trusted`] to remove these limits.
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
        Ok(Self {
            conf,
            connection: conn,
        })
    }

    /// Removes operational limits from this connection. (see [`Connection::untrusted`])
    #[must_use]
    pub fn trusted(self) -> Self {
        self.connection
            .authorizer::<fn(rusqlite::hooks::AuthContext<'_>) -> rusqlite::hooks::Authorization>(
                None,
            );
        self
    }

    // [tag:sync_auth_doc]
    /// Sets operational limits for this connection.
    ///
    /// - Allow `INSERT`, `DELETE` only for "queue", "candidate_subscription", "subscription".
    /// - Allow `UPDATE` only for "subscription" user facing settings.
    /// - Allow `INSERT` only for "post".
    /// - Allow read access to all tables.
    /// - Allow `SELECT`, `TRANSACTION`, `SAVEPOINT`, and the `strftime` function.
    /// - Deny everything else.
    pub fn untrusted(self) -> Self {
        self.connection.authorizer(Some(user_authorizer_callback));
        self
    }

    /// Create a database if it doesn't exist and then open it.
    pub fn open_or_create_db(conf: Configuration) -> Result<Self> {
        if !conf.db_path.exists() {
            let db_path = &conf.db_path;
            use std::os::unix::fs::PermissionsExt;

            info!("Creating database in {}", db_path.display());
            std::fs::File::create(db_path).context("Could not create db path")?;

            let mut child = Command::new("sqlite3")
                .arg(db_path)
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

            let file = std::fs::File::open(db_path)?;
            let metadata = file.metadata()?;
            let mut permissions = metadata.permissions();

            permissions.set_mode(0o600); // Read/write for owner only.
            file.set_permissions(permissions)?;
        }
        Self::open_db(conf)
    }

    /// Returns a connection's configuration.
    pub fn conf(&self) -> &Configuration {
        &self.conf
    }

    /// Loads archive databases from [`Configuration::data_path`], if any.
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

    /// Returns a vector of existing mailing lists.
    pub fn lists(&self) -> Result<Vec<DbVal<MailingList>>> {
        let mut stmt = self.connection.prepare("SELECT * FROM list;")?;
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

    /// Fetch a mailing list by primary key.
    pub fn list(&self, pk: i64) -> Result<DbVal<MailingList>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM list WHERE pk = ?;")?;
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
        ret.map_or_else(
            || Err(Error::from(NotFound("list or list policy not found!"))),
            Ok,
        )
    }

    /// Fetch a mailing list by id.
    pub fn list_by_id<S: AsRef<str>>(&self, id: S) -> Result<Option<DbVal<MailingList>>> {
        let id = id.as_ref();
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM list WHERE id = ?;")?;
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

    /// Create a new list.
    pub fn create_list(&self, new_val: MailingList) -> Result<DbVal<MailingList>> {
        let mut stmt = self
            .connection
            .prepare("INSERT INTO list(name, id, address, description, archive_url) VALUES(?, ?, ?, ?, ?) RETURNING *;")?;
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

    /// Fetch all posts of a mailing list.
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

    /// Fetch the owners of a mailing list.
    pub fn list_owners(&self, pk: i64) -> Result<Vec<DbVal<ListOwner>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM owner WHERE list = ?;")?;
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

    /// Remove an owner of a mailing list.
    pub fn remove_list_owner(&self, list_pk: i64, owner_pk: i64) -> Result<()> {
        self.connection
            .query_row(
                "DELETE FROM owner WHERE list = ? AND pk = ? RETURNING *;",
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

    /// Add an owner of a mailing list.
    pub fn add_list_owner(&self, list_owner: ListOwner) -> Result<DbVal<ListOwner>> {
        let mut stmt = self.connection.prepare(
            "INSERT OR REPLACE INTO owner(list, address, name) VALUES (?, ?, ?) RETURNING *;",
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

    /// Update a mailing list.
    pub fn update_list(&mut self, change_set: MailingListChangeset) -> Result<()> {
        if matches!(
            change_set,
            MailingListChangeset {
                pk: _,
                name: None,
                id: None,
                address: None,
                description: None,
                archive_url: None,
                owner_local_part: None,
                request_local_part: None,
                verify: None,
                hidden: None,
                enabled: None,
            }
        ) {
            return self.list(change_set.pk).map(|_| ());
        }

        let MailingListChangeset {
            pk,
            name,
            id,
            address,
            description,
            archive_url,
            owner_local_part,
            request_local_part,
            verify,
            hidden,
            enabled,
        } = change_set;
        let tx = self.connection.transaction()?;

        macro_rules! update {
            ($field:tt) => {{
                if let Some($field) = $field {
                    tx.execute(
                        concat!("UPDATE list SET ", stringify!($field), " = ? WHERE pk = ?;"),
                        rusqlite::params![&$field, &pk],
                    )?;
                }
            }};
        }
        update!(name);
        update!(id);
        update!(address);
        update!(description);
        update!(archive_url);
        update!(owner_local_part);
        update!(request_local_part);
        update!(verify);
        update!(hidden);
        update!(enabled);

        tx.commit()?;
        Ok(())
    }

    /// Return the post filters of a mailing list.
    pub fn list_filters(
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
