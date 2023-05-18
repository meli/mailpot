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

//! How each list handles new posts and new subscriptions.

pub use post_policy::*;
pub use subscription_policy::*;

mod post_policy {
    use log::trace;
    use rusqlite::OptionalExtension;

    use crate::{
        errors::{ErrorKind::*, *},
        models::{DbVal, PostPolicy},
        Connection,
    };

    impl Connection {
        /// Fetch the post policy of a mailing list.
        pub fn list_post_policy(&self, pk: i64) -> Result<Option<DbVal<PostPolicy>>> {
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
                            subscription_only: row.get("subscription_only")?,
                            approval_needed: row.get("approval_needed")?,
                            open: row.get("open")?,
                            custom: row.get("custom")?,
                        },
                        pk,
                    ))
                })
                .optional()?;

            Ok(ret)
        }

        /// Remove an existing list policy.
        ///
        /// ```
        /// # use mailpot::{models::*, Configuration, Connection, SendMail};
        /// # use tempfile::TempDir;
        ///
        /// # let tmp_dir = TempDir::new().unwrap();
        /// # let db_path = tmp_dir.path().join("mpot.db");
        /// # let config = Configuration {
        /// #     send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        /// #     db_path: db_path.clone(),
        /// #     data_path: tmp_dir.path().to_path_buf(),
        /// #     administrators: vec![],
        /// # };
        ///
        /// # fn do_test(config: Configuration) {
        /// let db = Connection::open_or_create_db(config).unwrap().trusted();
        /// # assert!(db.list_post_policy(1).unwrap().is_none());
        /// let list = db
        ///     .create_list(MailingList {
        ///         pk: 0,
        ///         name: "foobar chat".into(),
        ///         id: "foo-chat".into(),
        ///         address: "foo-chat@example.com".into(),
        ///         description: None,
        ///         topics: vec![],
        ///         archive_url: None,
        ///     })
        ///     .unwrap();
        ///
        /// # assert!(db.list_post_policy(list.pk()).unwrap().is_none());
        /// let pol = db
        ///     .set_list_post_policy(PostPolicy {
        ///         pk: -1,
        ///         list: list.pk(),
        ///         announce_only: false,
        ///         subscription_only: true,
        ///         approval_needed: false,
        ///         open: false,
        ///         custom: false,
        ///     })
        ///     .unwrap();
        /// # assert_eq!(db.list_post_policy(list.pk()).unwrap().as_ref(), Some(&pol));
        /// db.remove_list_post_policy(list.pk(), pol.pk()).unwrap();
        /// # assert!(db.list_post_policy(list.pk()).unwrap().is_none());
        /// # }
        /// # do_test(config);
        /// ```
        pub fn remove_list_post_policy(&self, list_pk: i64, policy_pk: i64) -> Result<()> {
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

            trace!("remove_list_post_policy {} {}.", list_pk, policy_pk);
            Ok(())
        }

        /// ```should_panic
        /// # use mailpot::{models::*, Configuration, Connection, SendMail};
        /// # use tempfile::TempDir;
        ///
        /// # let tmp_dir = TempDir::new().unwrap();
        /// # let db_path = tmp_dir.path().join("mpot.db");
        /// # let config = Configuration {
        /// #     send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        /// #     db_path: db_path.clone(),
        /// #     data_path: tmp_dir.path().to_path_buf(),
        /// #     administrators: vec![],
        /// # };
        ///
        /// # fn do_test(config: Configuration) {
        /// let db = Connection::open_or_create_db(config).unwrap().trusted();
        /// db.remove_list_post_policy(1, 1).unwrap();
        /// # }
        /// # do_test(config);
        /// ```
        #[cfg(doc)]
        pub fn remove_list_post_policy_panic() {}

        /// Set the unique post policy for a list.
        pub fn set_list_post_policy(&self, policy: PostPolicy) -> Result<DbVal<PostPolicy>> {
            if !(policy.announce_only
                || policy.subscription_only
                || policy.approval_needed
                || policy.open
                || policy.custom)
            {
                return Err(
                    "Cannot add empty policy. Having no policies is probably what you want to do."
                        .into(),
                );
            }
            let list_pk = policy.list;

            let mut stmt = self.connection.prepare(
                "INSERT OR REPLACE INTO post_policy(list, announce_only, subscription_only, \
                 approval_needed, open, custom) VALUES (?, ?, ?, ?, ?, ?) RETURNING *;",
            )?;
            let ret = stmt
                .query_row(
                    rusqlite::params![
                        &list_pk,
                        &policy.announce_only,
                        &policy.subscription_only,
                        &policy.approval_needed,
                        &policy.open,
                        &policy.custom,
                    ],
                    |row| {
                        let pk = row.get("pk")?;
                        Ok(DbVal(
                            PostPolicy {
                                pk,
                                list: row.get("list")?,
                                announce_only: row.get("announce_only")?,
                                subscription_only: row.get("subscription_only")?,
                                approval_needed: row.get("approval_needed")?,
                                open: row.get("open")?,
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
                        Error::from(err)
                            .chain_err(|| NotFound("Could not find a list with this pk."))
                    } else {
                        err.into()
                    }
                })?;

            trace!("set_list_post_policy {:?}.", &ret);
            Ok(ret)
        }
    }
}

mod subscription_policy {
    use log::trace;
    use rusqlite::OptionalExtension;

    use crate::{
        errors::{ErrorKind::*, *},
        models::{DbVal, SubscriptionPolicy},
        Connection,
    };

    impl Connection {
        /// Fetch the subscription policy of a mailing list.
        pub fn list_subscription_policy(
            &self,
            pk: i64,
        ) -> Result<Option<DbVal<SubscriptionPolicy>>> {
            let mut stmt = self
                .connection
                .prepare("SELECT * FROM subscription_policy WHERE list = ?;")?;
            let ret = stmt
                .query_row([&pk], |row| {
                    let pk = row.get("pk")?;
                    Ok(DbVal(
                        SubscriptionPolicy {
                            pk,
                            list: row.get("list")?,
                            send_confirmation: row.get("send_confirmation")?,
                            open: row.get("open")?,
                            manual: row.get("manual")?,
                            request: row.get("request")?,
                            custom: row.get("custom")?,
                        },
                        pk,
                    ))
                })
                .optional()?;

            Ok(ret)
        }

        /// Remove an existing subscription policy.
        ///
        /// ```
        /// # use mailpot::{models::*, Configuration, Connection, SendMail};
        /// # use tempfile::TempDir;
        ///
        /// # let tmp_dir = TempDir::new().unwrap();
        /// # let db_path = tmp_dir.path().join("mpot.db");
        /// # let config = Configuration {
        /// #     send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        /// #     db_path: db_path.clone(),
        /// #     data_path: tmp_dir.path().to_path_buf(),
        /// #     administrators: vec![],
        /// # };
        ///
        /// # fn do_test(config: Configuration) {
        /// let db = Connection::open_or_create_db(config).unwrap().trusted();
        /// let list = db
        ///     .create_list(MailingList {
        ///         pk: 0,
        ///         name: "foobar chat".into(),
        ///         id: "foo-chat".into(),
        ///         address: "foo-chat@example.com".into(),
        ///         description: None,
        ///         topics: vec![],
        ///         archive_url: None,
        ///     })
        ///     .unwrap();
        /// # assert!(db.list_subscription_policy(list.pk()).unwrap().is_none());
        /// let pol = db
        ///     .set_list_subscription_policy(SubscriptionPolicy {
        ///         pk: -1,
        ///         list: list.pk(),
        ///         send_confirmation: false,
        ///         open: true,
        ///         manual: false,
        ///         request: false,
        ///         custom: false,
        ///     })
        ///     .unwrap();
        /// # assert_eq!(db.list_subscription_policy(list.pk()).unwrap().as_ref(), Some(&pol));
        /// db.remove_list_subscription_policy(list.pk(), pol.pk())
        ///     .unwrap();
        /// # assert!(db.list_subscription_policy(list.pk()).unwrap().is_none());
        /// # }
        /// # do_test(config);
        /// ```
        pub fn remove_list_subscription_policy(&self, list_pk: i64, policy_pk: i64) -> Result<()> {
            let mut stmt = self.connection.prepare(
                "DELETE FROM subscription_policy WHERE pk = ? AND list = ? RETURNING *;",
            )?;
            stmt.query_row(rusqlite::params![&policy_pk, &list_pk,], |_| Ok(()))
                .map_err(|err| {
                    if matches!(err, rusqlite::Error::QueryReturnedNoRows) {
                        Error::from(err).chain_err(|| NotFound("list or list policy not found!"))
                    } else {
                        err.into()
                    }
                })?;

            trace!("remove_list_subscription_policy {} {}.", list_pk, policy_pk);
            Ok(())
        }

        /// ```should_panic
        /// # use mailpot::{models::*, Configuration, Connection, SendMail};
        /// # use tempfile::TempDir;
        ///
        /// # let tmp_dir = TempDir::new().unwrap();
        /// # let db_path = tmp_dir.path().join("mpot.db");
        /// # let config = Configuration {
        /// #     send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
        /// #     db_path: db_path.clone(),
        /// #     data_path: tmp_dir.path().to_path_buf(),
        /// #     administrators: vec![],
        /// # };
        ///
        /// # fn do_test(config: Configuration) {
        /// let db = Connection::open_or_create_db(config).unwrap().trusted();
        /// db.remove_list_post_policy(1, 1).unwrap();
        /// # }
        /// # do_test(config);
        /// ```
        #[cfg(doc)]
        pub fn remove_list_subscription_policy_panic() {}

        /// Set the unique post policy for a list.
        pub fn set_list_subscription_policy(
            &self,
            policy: SubscriptionPolicy,
        ) -> Result<DbVal<SubscriptionPolicy>> {
            if !(policy.open || policy.manual || policy.request || policy.custom) {
                return Err(
                    "Cannot add empty policy. Having no policy is probably what you want to do."
                        .into(),
                );
            }
            let list_pk = policy.list;

            let mut stmt = self.connection.prepare(
                "INSERT OR REPLACE INTO subscription_policy(list, send_confirmation, open, \
                 manual, request, custom) VALUES (?, ?, ?, ?, ?, ?) RETURNING *;",
            )?;
            let ret = stmt
                .query_row(
                    rusqlite::params![
                        &list_pk,
                        &policy.send_confirmation,
                        &policy.open,
                        &policy.manual,
                        &policy.request,
                        &policy.custom,
                    ],
                    |row| {
                        let pk = row.get("pk")?;
                        Ok(DbVal(
                            SubscriptionPolicy {
                                pk,
                                list: row.get("list")?,
                                send_confirmation: row.get("send_confirmation")?,
                                open: row.get("open")?,
                                manual: row.get("manual")?,
                                request: row.get("request")?,
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
                        Error::from(err)
                            .chain_err(|| NotFound("Could not find a list with this pk."))
                    } else {
                        err.into()
                    }
                })?;

            trace!("set_list_subscription_policy {:?}.", &ret);
            Ok(ret)
        }
    }
}
