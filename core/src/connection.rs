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

use std::{
    io::Write,
    process::{Command, Stdio},
};

use jsonschema::JSONSchema;
use log::{info, trace};
use rusqlite::{functions::FunctionFlags, Connection as DbConnection, OptionalExtension};

use crate::{
    config::Configuration,
    errors::{ErrorKind::*, *},
    models::{changesets::MailingListChangeset, DbVal, ListOwner, MailingList, Post},
};

/// A connection to a `mailpot` database.
pub struct Connection {
    /// The `rusqlite` connection handle.
    pub connection: DbConnection,
    pub(crate) conf: Configuration,
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Connection")
            .field("conf", &self.conf)
            .finish()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.connection
            .authorizer::<fn(rusqlite::hooks::AuthContext<'_>) -> rusqlite::hooks::Authorization>(
                None,
            );
        // make sure pragma optimize does not take too long
        _ = self.connection.pragma_update(None, "analysis_limit", "400");
        // gather statistics to improve query optimization
        _ = self
            .connection
            .pragma(None, "optimize", 0xfffe_i64, |_| Ok(()));
    }
}

fn log_callback(error_code: std::ffi::c_int, message: &str) {
    match error_code {
        rusqlite::ffi::SQLITE_NOTICE => log::trace!("{}", message),
        rusqlite::ffi::SQLITE_OK
        | rusqlite::ffi::SQLITE_DONE
        | rusqlite::ffi::SQLITE_NOTICE_RECOVER_WAL
        | rusqlite::ffi::SQLITE_NOTICE_RECOVER_ROLLBACK => log::info!("{}", message),
        rusqlite::ffi::SQLITE_WARNING | rusqlite::ffi::SQLITE_WARNING_AUTOINDEX => {
            log::warn!("{}", message)
        }
        _ => log::error!("{error_code} {}", message),
    }
}

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
            table_name: "candidate_subscription" | "template",
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
            function_name: "count" | "strftime" | "unixepoch" | "datetime",
        } => Authorization::Allow,
        _ => Authorization::Deny,
    }
}

impl Connection {
    /// The database schema.
    ///
    /// ```sql
    #[doc = include_str!("./schema.sql")]
    /// ```
    pub const SCHEMA: &'static str = include_str!("./schema.sql");

    /// Database migrations.
    pub const MIGRATIONS: &'static [(u32, &'static str, &'static str)] =
        include!("./migrations.rs.inc");

    /// Creates a new database connection.
    ///
    /// `Connection` supports a limited subset of operations by default (see
    /// [`Connection::untrusted`]).
    /// Use [`Connection::trusted`] to remove these limits.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use mailpot::{Connection, Configuration};
    /// use melib::smtp::{SmtpServerConf, SmtpAuth, SmtpSecurity};
    /// #
    /// # fn main() -> mailpot::Result<()> {
    /// # use tempfile::TempDir;
    /// #
    /// # let tmp_dir = TempDir::new()?;
    /// # let db_path = tmp_dir.path().join("mpot.db");
    /// # let data_path = tmp_dir.path().to_path_buf();
    /// let config = Configuration {
    ///     send_mail: mailpot::SendMail::Smtp(
    ///         SmtpServerConf {
    ///             hostname: "127.0.0.1".into(),
    ///             port: 25,
    ///             envelope_from: "foo-chat@example.com".into(),
    ///             auth: SmtpAuth::None,
    ///             security: SmtpSecurity::None,
    ///             extensions: Default::default(),
    ///         }
    ///     ),
    ///     db_path,
    ///     data_path,
    ///     administrators: vec![],
    /// };
    /// # assert_eq!(&Connection::open_db(config.clone()).unwrap_err().to_string(), "Database doesn't exist");
    ///
    /// let db = Connection::open_or_create_db(config)?;
    /// # _ = db;
    /// # Ok(())
    /// # }
    /// ```
    pub fn open_db(conf: Configuration) -> Result<Self> {
        use std::sync::Once;

        use rusqlite::config::DbConfig;

        static INIT_SQLITE_LOGGING: Once = Once::new();

        if !conf.db_path.exists() {
            return Err("Database doesn't exist".into());
        }
        INIT_SQLITE_LOGGING.call_once(|| {
            _ = unsafe { rusqlite::trace::config_log(Some(log_callback)) };
        });
        let conn = DbConnection::open(conf.db_path.to_str().unwrap()).with_context(|| {
            format!("sqlite3 library could not open {}.", conf.db_path.display())
        })?;
        rusqlite::vtab::array::load_module(&conn)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "on")?;
        // synchronise less often to the filesystem
        conn.pragma_update(None, "synchronous", "normal")?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY, true)?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, true)?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, true)?;
        conn.busy_timeout(core::time::Duration::from_millis(500))?;
        conn.busy_handler(Some(|times: i32| -> bool { times < 5 }))?;
        conn.create_scalar_function(
            "validate_json_schema",
            2,
            FunctionFlags::SQLITE_INNOCUOUS
                | FunctionFlags::SQLITE_UTF8
                | FunctionFlags::SQLITE_DETERMINISTIC,
            |ctx| {
                if log::log_enabled!(log::Level::Trace) {
                    rusqlite::trace::log(
                        rusqlite::ffi::SQLITE_NOTICE,
                        "validate_json_schema RUNNING",
                    );
                }
                let map_err = rusqlite::Error::UserFunctionError;
                let schema = ctx.get::<String>(0)?;
                let value = ctx.get::<String>(1)?;
                let schema_val: serde_json::Value = serde_json::from_str(&schema)
                    .map_err(Into::into)
                    .map_err(map_err)?;
                let value: serde_json::Value = serde_json::from_str(&value)
                    .map_err(Into::into)
                    .map_err(map_err)?;
                let compiled = JSONSchema::compile(&schema_val)
                    .map_err(|err| err.to_string())
                    .map_err(Into::into)
                    .map_err(map_err)?;
                let x = if let Err(errors) = compiled.validate(&value) {
                    for err in errors {
                        rusqlite::trace::log(rusqlite::ffi::SQLITE_WARNING, &err.to_string());
                        drop(err);
                    }
                    Ok(false)
                } else {
                    Ok(true)
                };
                x
            },
        )?;

        let ret = Self {
            conf,
            connection: conn,
        };
        if let Some(&(latest, _, _)) = Self::MIGRATIONS.last() {
            let version = ret.schema_version()?;
            trace!(
                "SQLITE user_version PRAGMA returned {version}. Most recent migration is {latest}."
            );
            if version < latest {
                info!("Updating database schema from version {version} to {latest}...");
            }
            ret.migrate(version, latest)?;
        }

        ret.connection.authorizer(Some(user_authorizer_callback));
        Ok(ret)
    }

    /// The version of the current schema.
    pub fn schema_version(&self) -> Result<u32> {
        Ok(self
            .connection
            .prepare("SELECT user_version FROM pragma_user_version;")?
            .query_row([], |row| {
                let v: u32 = row.get(0)?;
                Ok(v)
            })?)
    }

    /// Migrate from version `from` to `to`.
    ///
    /// See [Self::MIGRATIONS].
    pub fn migrate(&self, mut from: u32, to: u32) -> Result<()> {
        if from == to {
            return Ok(());
        }

        let undo = from > to;
        let tx = self.savepoint(Some(stringify!(migrate)))?;

        while from != to {
            log::trace!(
                "exec migration from {from} to {to}, type: {}do",
                if undo { "un " } else { "re" }
            );
            if undo {
                trace!("{}", Self::MIGRATIONS[from as usize - 1].2);
                tx.connection
                    .execute_batch(Self::MIGRATIONS[from as usize - 1].2)?;
                from -= 1;
            } else {
                trace!("{}", Self::MIGRATIONS[from as usize].1);
                tx.connection
                    .execute_batch(Self::MIGRATIONS[from as usize].1)?;
                from += 1;
            }
        }
        tx.connection
            .pragma_update(None, "user_version", Self::MIGRATIONS[to as usize - 1].0)?;

        tx.commit()?;

        Ok(())
    }

    /// Removes operational limits from this connection. (see
    /// [`Connection::untrusted`])
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
    /// - Allow `INSERT`, `DELETE` only for "queue", "candidate_subscription",
    ///   "subscription".
    /// - Allow `UPDATE` only for "subscription" user facing settings.
    /// - Allow `INSERT` only for "post".
    /// - Allow read access to all tables.
    /// - Allow `SELECT`, `TRANSACTION`, `SAVEPOINT`, and the `strftime`
    ///   function.
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

            let mut child =
                Command::new(std::env::var("SQLITE_BIN").unwrap_or_else(|_| "sqlite3".into()))
                    .arg(db_path)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .with_context(|| {
                        format!(
                            "Could not launch {} {}.",
                            std::env::var("SQLITE_BIN").unwrap_or_else(|_| "sqlite3".into()),
                            db_path.display()
                        )
                    })?;
            let mut stdin = child.stdin.take().unwrap();
            std::thread::spawn(move || {
                stdin
                    .write_all(Self::SCHEMA.as_bytes())
                    .expect("failed to write to stdin");
                if !Self::MIGRATIONS.is_empty() {
                    stdin
                        .write_all(b"\nPRAGMA user_version = ")
                        .expect("failed to write to stdin");
                    stdin
                        .write_all(
                            Self::MIGRATIONS[Self::MIGRATIONS.len() - 1]
                                .0
                                .to_string()
                                .as_bytes(),
                        )
                        .expect("failed to write to stdin");
                    stdin.write_all(b";").expect("failed to write to stdin");
                }
                stdin.flush().expect("could not flush stdin");
            });
            let output = child.wait_with_output()?;
            if !output.status.success() {
                return Err(format!(
                    "Could not initialize sqlite3 database at {}: sqlite3 returned exit code {} \
                     and stderr {} {}",
                    db_path.display(),
                    output.status.code().unwrap_or_default(),
                    String::from_utf8_lossy(&output.stderr),
                    String::from_utf8_lossy(&output.stdout)
                )
                .into());
            }

            let file = std::fs::File::open(db_path)
                .with_context(|| format!("Could not open database {}.", db_path.display()))?;
            let metadata = file
                .metadata()
                .with_context(|| format!("Could not fstat database {}.", db_path.display()))?;
            let mut permissions = metadata.permissions();

            permissions.set_mode(0o600); // Read/write for owner only.
            file.set_permissions(permissions)
                .with_context(|| format!("Could not chmod 600 database {}.", db_path.display()))?;
        }
        Self::open_db(conf)
    }

    /// Returns a connection's configuration.
    pub fn conf(&self) -> &Configuration {
        &self.conf
    }

    /// Loads archive databases from [`Configuration::data_path`], if any.
    pub fn load_archives(&self) -> Result<()> {
        let tx = self.savepoint(Some(stringify!(load_archives)))?;
        {
            let mut stmt = tx.connection.prepare("ATTACH ? AS ?;")?;
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
        }
        tx.commit()?;

        Ok(())
    }

    /// Returns a vector of existing mailing lists.
    pub fn lists(&self) -> Result<Vec<DbVal<MailingList>>> {
        let mut stmt = self.connection.prepare("SELECT * FROM list;")?;
        let list_iter = stmt.query_map([], |row| {
            let pk = row.get("pk")?;
            let topics: serde_json::Value = row.get("topics")?;
            let topics = MailingList::topics_from_json_value(topics)?;
            Ok(DbVal(
                MailingList {
                    pk,
                    name: row.get("name")?,
                    id: row.get("id")?,
                    address: row.get("address")?,
                    description: row.get("description")?,
                    topics,
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
    pub fn list(&self, pk: i64) -> Result<Option<DbVal<MailingList>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM list WHERE pk = ?;")?;
        let ret = stmt
            .query_row([&pk], |row| {
                let pk = row.get("pk")?;
                let topics: serde_json::Value = row.get("topics")?;
                let topics = MailingList::topics_from_json_value(topics)?;
                Ok(DbVal(
                    MailingList {
                        pk,
                        name: row.get("name")?,
                        id: row.get("id")?,
                        address: row.get("address")?,
                        description: row.get("description")?,
                        topics,
                        archive_url: row.get("archive_url")?,
                    },
                    pk,
                ))
            })
            .optional()?;
        Ok(ret)
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
                let topics: serde_json::Value = row.get("topics")?;
                let topics = MailingList::topics_from_json_value(topics)?;
                Ok(DbVal(
                    MailingList {
                        pk,
                        name: row.get("name")?,
                        id: row.get("id")?,
                        address: row.get("address")?,
                        description: row.get("description")?,
                        topics,
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
        let mut stmt = self.connection.prepare(
            "INSERT INTO list(name, id, address, description, archive_url, topics) VALUES(?, ?, \
             ?, ?, ?, ?) RETURNING *;",
        )?;
        let ret = stmt.query_row(
            rusqlite::params![
                &new_val.name,
                &new_val.id,
                &new_val.address,
                new_val.description.as_ref(),
                new_val.archive_url.as_ref(),
                serde_json::json!(new_val.topics.as_slice()),
            ],
            |row| {
                let pk = row.get("pk")?;
                let topics: serde_json::Value = row.get("topics")?;
                let topics = MailingList::topics_from_json_value(topics)?;
                Ok(DbVal(
                    MailingList {
                        pk,
                        name: row.get("name")?,
                        id: row.get("id")?,
                        address: row.get("address")?,
                        description: row.get("description")?,
                        topics,
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
        let mut stmt = self.connection.prepare(
            "SELECT *, strftime('%Y-%m', CAST(timestamp AS INTEGER), 'unixepoch') AS month_year \
             FROM post WHERE list = ? ORDER BY timestamp ASC;",
        )?;
        let iter = stmt.query_map(rusqlite::params![&list_pk], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                Post {
                    pk,
                    list: row.get("list")?,
                    envelope_from: row.get("envelope_from")?,
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
                    Error::from(err)
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
    pub fn update_list(&self, change_set: MailingListChangeset) -> Result<()> {
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
        let tx = self.savepoint(Some(stringify!(update_list)))?;

        macro_rules! update {
            ($field:tt) => {{
                if let Some($field) = $field {
                    tx.connection.execute(
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

    /// Execute operations inside an SQL transaction.
    pub fn transaction(
        &'_ mut self,
        behavior: transaction::TransactionBehavior,
    ) -> Result<transaction::Transaction<'_>> {
        use transaction::*;

        let query = match behavior {
            TransactionBehavior::Deferred => "BEGIN DEFERRED",
            TransactionBehavior::Immediate => "BEGIN IMMEDIATE",
            TransactionBehavior::Exclusive => "BEGIN EXCLUSIVE",
        };
        self.connection.execute_batch(query)?;
        Ok(Transaction {
            conn: self,
            drop_behavior: DropBehavior::Rollback,
        })
    }

    /// Execute operations inside an SQL savepoint.
    pub fn savepoint(&'_ self, name: Option<&'static str>) -> Result<transaction::Savepoint<'_>> {
        use std::sync::atomic::{AtomicUsize, Ordering};

        use transaction::*;
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        let name = name
            .map(Ok)
            .unwrap_or_else(|| Err(COUNTER.fetch_add(1, Ordering::Relaxed)));

        match name {
            Ok(ref n) => self.connection.execute_batch(&format!("SAVEPOINT {n}"))?,
            Err(ref i) => self.connection.execute_batch(&format!("SAVEPOINT _{i}"))?,
        };

        Ok(Savepoint {
            conn: self,
            drop_behavior: DropBehavior::Rollback,
            name,
            committed: false,
        })
    }
}

/// Execute operations inside an SQL transaction.
pub mod transaction {
    use super::*;

    /// A transaction handle.
    #[derive(Debug)]
    pub struct Transaction<'conn> {
        pub(super) conn: &'conn mut Connection,
        pub(super) drop_behavior: DropBehavior,
    }

    impl Drop for Transaction<'_> {
        fn drop(&mut self) {
            _ = self.finish_();
        }
    }

    impl Transaction<'_> {
        /// Commit and consume transaction.
        pub fn commit(mut self) -> Result<()> {
            self.commit_()
        }

        fn commit_(&mut self) -> Result<()> {
            self.conn.connection.execute_batch("COMMIT")?;
            Ok(())
        }

        /// Configure the transaction to perform the specified action when it is
        /// dropped.
        #[inline]
        pub fn set_drop_behavior(&mut self, drop_behavior: DropBehavior) {
            self.drop_behavior = drop_behavior;
        }

        /// A convenience method which consumes and rolls back a transaction.
        #[inline]
        pub fn rollback(mut self) -> Result<()> {
            self.rollback_()
        }

        fn rollback_(&mut self) -> Result<()> {
            self.conn.connection.execute_batch("ROLLBACK")?;
            Ok(())
        }

        /// Consumes the transaction, committing or rolling back according to
        /// the current setting (see `drop_behavior`).
        ///
        /// Functionally equivalent to the `Drop` implementation, but allows
        /// callers to see any errors that occur.
        #[inline]
        pub fn finish(mut self) -> Result<()> {
            self.finish_()
        }

        #[inline]
        fn finish_(&mut self) -> Result<()> {
            if self.conn.connection.is_autocommit() {
                return Ok(());
            }
            match self.drop_behavior {
                DropBehavior::Commit => self.commit_().or_else(|_| self.rollback_()),
                DropBehavior::Rollback => self.rollback_(),
                DropBehavior::Ignore => Ok(()),
                DropBehavior::Panic => panic!("Transaction dropped unexpectedly."),
            }
        }
    }

    impl std::ops::Deref for Transaction<'_> {
        type Target = Connection;

        #[inline]
        fn deref(&self) -> &Connection {
            self.conn
        }
    }

    /// Options for transaction behavior. See [BEGIN
    /// TRANSACTION](http://www.sqlite.org/lang_transaction.html) for details.
    #[derive(Copy, Clone, Default)]
    #[non_exhaustive]
    pub enum TransactionBehavior {
        /// DEFERRED means that the transaction does not actually start until
        /// the database is first accessed.
        Deferred,
        #[default]
        /// IMMEDIATE cause the database connection to start a new write
        /// immediately, without waiting for a writes statement.
        Immediate,
        /// EXCLUSIVE prevents other database connections from reading the
        /// database while the transaction is underway.
        Exclusive,
    }

    /// Options for how a Transaction or Savepoint should behave when it is
    /// dropped.
    #[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
    #[non_exhaustive]
    pub enum DropBehavior {
        #[default]
        /// Roll back the changes. This is the default.
        Rollback,

        /// Commit the changes.
        Commit,

        /// Do not commit or roll back changes - this will leave the transaction
        /// or savepoint open, so should be used with care.
        Ignore,

        /// Panic. Used to enforce intentional behavior during development.
        Panic,
    }

    /// A savepoint handle.
    #[derive(Debug)]
    pub struct Savepoint<'conn> {
        pub(super) conn: &'conn Connection,
        pub(super) drop_behavior: DropBehavior,
        pub(super) name: std::result::Result<&'static str, usize>,
        pub(super) committed: bool,
    }

    impl Drop for Savepoint<'_> {
        fn drop(&mut self) {
            _ = self.finish_();
        }
    }

    impl Savepoint<'_> {
        /// Commit and consume savepoint.
        pub fn commit(mut self) -> Result<()> {
            self.commit_()
        }

        fn commit_(&mut self) -> Result<()> {
            if !self.committed {
                match self.name {
                    Ok(ref n) => self
                        .conn
                        .connection
                        .execute_batch(&format!("RELEASE SAVEPOINT {n}"))?,
                    Err(ref i) => self
                        .conn
                        .connection
                        .execute_batch(&format!("RELEASE SAVEPOINT _{i}"))?,
                };
                self.committed = true;
            }
            Ok(())
        }

        /// Configure the savepoint to perform the specified action when it is
        /// dropped.
        #[inline]
        pub fn set_drop_behavior(&mut self, drop_behavior: DropBehavior) {
            self.drop_behavior = drop_behavior;
        }

        /// A convenience method which consumes and rolls back a savepoint.
        #[inline]
        pub fn rollback(mut self) -> Result<()> {
            self.rollback_()
        }

        fn rollback_(&mut self) -> Result<()> {
            if !self.committed {
                match self.name {
                    Ok(ref n) => self
                        .conn
                        .connection
                        .execute_batch(&format!("ROLLBACK TO SAVEPOINT {n}"))?,
                    Err(ref i) => self
                        .conn
                        .connection
                        .execute_batch(&format!("ROLLBACK TO SAVEPOINT _{i}"))?,
                };
            }
            Ok(())
        }

        /// Consumes the savepoint, committing or rolling back according to
        /// the current setting (see `drop_behavior`).
        ///
        /// Functionally equivalent to the `Drop` implementation, but allows
        /// callers to see any errors that occur.
        #[inline]
        pub fn finish(mut self) -> Result<()> {
            self.finish_()
        }

        #[inline]
        fn finish_(&mut self) -> Result<()> {
            if self.conn.connection.is_autocommit() {
                return Ok(());
            }
            match self.drop_behavior {
                DropBehavior::Commit => self.commit_().or_else(|_| self.rollback_()),
                DropBehavior::Rollback => self.rollback_(),
                DropBehavior::Ignore => Ok(()),
                DropBehavior::Panic => panic!("Savepoint dropped unexpectedly."),
            }
        }
    }

    impl std::ops::Deref for Savepoint<'_> {
        type Target = Connection;

        #[inline]
        fn deref(&self) -> &Connection {
            self.conn
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_connection() {
        use melib::smtp::{SmtpAuth, SmtpSecurity, SmtpServerConf};
        use tempfile::TempDir;

        use crate::SendMail;

        let tmp_dir = TempDir::new().unwrap();
        let db_path = tmp_dir.path().join("mpot.db");
        let data_path = tmp_dir.path().to_path_buf();
        let config = Configuration {
            send_mail: SendMail::Smtp(SmtpServerConf {
                hostname: "127.0.0.1".into(),
                port: 25,
                envelope_from: "foo-chat@example.com".into(),
                auth: SmtpAuth::None,
                security: SmtpSecurity::None,
                extensions: Default::default(),
            }),
            db_path,
            data_path,
            administrators: vec![],
        };
        assert_eq!(
            &Connection::open_db(config.clone()).unwrap_err().to_string(),
            "Database doesn't exist"
        );

        _ = Connection::open_or_create_db(config).unwrap();
    }

    #[test]
    fn test_transactions() {
        use melib::smtp::{SmtpAuth, SmtpSecurity, SmtpServerConf};
        use tempfile::TempDir;

        use super::transaction::*;
        use crate::SendMail;

        let tmp_dir = TempDir::new().unwrap();
        let db_path = tmp_dir.path().join("mpot.db");
        let data_path = tmp_dir.path().to_path_buf();
        let config = Configuration {
            send_mail: SendMail::Smtp(SmtpServerConf {
                hostname: "127.0.0.1".into(),
                port: 25,
                envelope_from: "foo-chat@example.com".into(),
                auth: SmtpAuth::None,
                security: SmtpSecurity::None,
                extensions: Default::default(),
            }),
            db_path,
            data_path,
            administrators: vec![],
        };
        let list = MailingList {
            pk: 0,
            name: "".into(),
            id: "".into(),
            description: None,
            topics: vec![],
            address: "".into(),
            archive_url: None,
        };
        let mut db = Connection::open_or_create_db(config).unwrap().trusted();

        /* drop rollback */
        let mut tx = db.transaction(Default::default()).unwrap();
        tx.set_drop_behavior(DropBehavior::Rollback);
        let _new = tx.create_list(list.clone()).unwrap();
        drop(tx);
        assert_eq!(&db.lists().unwrap(), &[]);

        /* drop commit */
        let mut tx = db.transaction(Default::default()).unwrap();
        tx.set_drop_behavior(DropBehavior::Commit);
        let new = tx.create_list(list.clone()).unwrap();
        drop(tx);
        assert_eq!(&db.lists().unwrap(), &[new.clone()]);

        /* rollback with drop commit */
        let mut tx = db.transaction(Default::default()).unwrap();
        tx.set_drop_behavior(DropBehavior::Commit);
        let _new2 = tx
            .create_list(MailingList {
                id: "1".into(),
                address: "1".into(),
                ..list.clone()
            })
            .unwrap();
        tx.rollback().unwrap();
        assert_eq!(&db.lists().unwrap(), &[new.clone()]);

        /* tx and then savepoint */
        let tx = db.transaction(Default::default()).unwrap();
        let sv = tx.savepoint(None).unwrap();
        let new2 = sv
            .create_list(MailingList {
                id: "2".into(),
                address: "2".into(),
                ..list.clone()
            })
            .unwrap();
        sv.commit().unwrap();
        tx.commit().unwrap();
        assert_eq!(&db.lists().unwrap(), &[new.clone(), new2.clone()]);

        /* tx and then rollback savepoint */
        let tx = db.transaction(Default::default()).unwrap();
        let sv = tx.savepoint(None).unwrap();
        let _new3 = sv
            .create_list(MailingList {
                id: "3".into(),
                address: "3".into(),
                ..list.clone()
            })
            .unwrap();
        sv.rollback().unwrap();
        tx.commit().unwrap();
        assert_eq!(&db.lists().unwrap(), &[new.clone(), new2.clone()]);

        /* tx, commit savepoint and then rollback commit */
        let tx = db.transaction(Default::default()).unwrap();
        let sv = tx.savepoint(None).unwrap();
        let _new3 = sv
            .create_list(MailingList {
                id: "3".into(),
                address: "3".into(),
                ..list.clone()
            })
            .unwrap();
        sv.commit().unwrap();
        tx.rollback().unwrap();
        assert_eq!(&db.lists().unwrap(), &[new.clone(), new2.clone()]);

        /* nested savepoints */
        let tx = db.transaction(Default::default()).unwrap();
        let sv = tx.savepoint(None).unwrap();
        let sv1 = sv.savepoint(None).unwrap();
        let new3 = sv1
            .create_list(MailingList {
                id: "3".into(),
                address: "3".into(),
                ..list
            })
            .unwrap();
        sv1.commit().unwrap();
        sv.commit().unwrap();
        tx.commit().unwrap();
        assert_eq!(&db.lists().unwrap(), &[new, new2, new3]);
    }
}
