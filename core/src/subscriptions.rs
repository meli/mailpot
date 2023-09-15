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

//! User subscriptions.

use log::trace;
use rusqlite::OptionalExtension;

use crate::{
    errors::*,
    models::{
        changesets::{AccountChangeset, ListSubscriptionChangeset},
        Account, ListCandidateSubscription, ListSubscription,
    },
    Connection, DbVal,
};

impl Connection {
    /// Fetch all subscriptions of a mailing list.
    pub fn list_subscriptions(&self, pk: i64) -> Result<Vec<DbVal<ListSubscription>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM subscription WHERE list = ?;")?;
        let list_iter = stmt.query_map([&pk], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                ListSubscription {
                    pk: row.get("pk")?,
                    list: row.get("list")?,
                    address: row.get("address")?,
                    account: row.get("account")?,
                    name: row.get("name")?,
                    digest: row.get("digest")?,
                    enabled: row.get("enabled")?,
                    verified: row.get("verified")?,
                    hide_address: row.get("hide_address")?,
                    receive_duplicates: row.get("receive_duplicates")?,
                    receive_own_posts: row.get("receive_own_posts")?,
                    receive_confirmation: row.get("receive_confirmation")?,
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

    /// Fetch mailing list subscription.
    pub fn list_subscription(&self, list_pk: i64, pk: i64) -> Result<DbVal<ListSubscription>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM subscription WHERE list = ? AND pk = ?;")?;

        let ret = stmt.query_row([&list_pk, &pk], |row| {
            let _pk: i64 = row.get("pk")?;
            debug_assert_eq!(pk, _pk);
            Ok(DbVal(
                ListSubscription {
                    pk,
                    list: row.get("list")?,
                    address: row.get("address")?,
                    account: row.get("account")?,
                    name: row.get("name")?,
                    digest: row.get("digest")?,
                    enabled: row.get("enabled")?,
                    verified: row.get("verified")?,
                    hide_address: row.get("hide_address")?,
                    receive_duplicates: row.get("receive_duplicates")?,
                    receive_own_posts: row.get("receive_own_posts")?,
                    receive_confirmation: row.get("receive_confirmation")?,
                },
                pk,
            ))
        })?;
        Ok(ret)
    }

    /// Fetch mailing list subscription by their address.
    pub fn list_subscription_by_address(
        &self,
        list_pk: i64,
        address: &str,
    ) -> Result<DbVal<ListSubscription>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM subscription WHERE list = ? AND address = ?;")?;

        let ret = stmt.query_row(rusqlite::params![&list_pk, &address], |row| {
            let pk = row.get("pk")?;
            let address_ = row.get("address")?;
            debug_assert_eq!(address, &address_);
            Ok(DbVal(
                ListSubscription {
                    pk,
                    list: row.get("list")?,
                    address: address_,
                    account: row.get("account")?,
                    name: row.get("name")?,
                    digest: row.get("digest")?,
                    enabled: row.get("enabled")?,
                    verified: row.get("verified")?,
                    hide_address: row.get("hide_address")?,
                    receive_duplicates: row.get("receive_duplicates")?,
                    receive_own_posts: row.get("receive_own_posts")?,
                    receive_confirmation: row.get("receive_confirmation")?,
                },
                pk,
            ))
        })?;
        Ok(ret)
    }

    /// Add subscription to mailing list.
    pub fn add_subscription(
        &self,
        list_pk: i64,
        mut new_val: ListSubscription,
    ) -> Result<DbVal<ListSubscription>> {
        new_val.list = list_pk;
        let mut stmt = self
            .connection
            .prepare(
                "INSERT INTO subscription(list, address, account, name, enabled, digest, \
                 verified, hide_address, receive_duplicates, receive_own_posts, \
                 receive_confirmation) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *;",
            )
            .unwrap();
        let val = stmt.query_row(
            rusqlite::params![
                &new_val.list,
                &new_val.address,
                &new_val.account,
                &new_val.name,
                &new_val.enabled,
                &new_val.digest,
                &new_val.verified,
                &new_val.hide_address,
                &new_val.receive_duplicates,
                &new_val.receive_own_posts,
                &new_val.receive_confirmation
            ],
            |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    ListSubscription {
                        pk,
                        list: row.get("list")?,
                        address: row.get("address")?,
                        name: row.get("name")?,
                        account: row.get("account")?,
                        digest: row.get("digest")?,
                        enabled: row.get("enabled")?,
                        verified: row.get("verified")?,
                        hide_address: row.get("hide_address")?,
                        receive_duplicates: row.get("receive_duplicates")?,
                        receive_own_posts: row.get("receive_own_posts")?,
                        receive_confirmation: row.get("receive_confirmation")?,
                    },
                    pk,
                ))
            },
        )?;
        trace!("add_subscription {:?}.", &val);
        // table entry might be modified by triggers, so don't rely on RETURNING value.
        self.list_subscription(list_pk, val.pk())
    }

    /// Create subscription candidate.
    pub fn add_candidate_subscription(
        &self,
        list_pk: i64,
        mut new_val: ListSubscription,
    ) -> Result<DbVal<ListCandidateSubscription>> {
        new_val.list = list_pk;
        let mut stmt = self.connection.prepare(
            "INSERT INTO candidate_subscription(list, address, name, accepted) VALUES(?, ?, ?, ?) \
             RETURNING *;",
        )?;
        let val = stmt.query_row(
            rusqlite::params![&new_val.list, &new_val.address, &new_val.name, None::<i64>,],
            |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    ListCandidateSubscription {
                        pk,
                        list: row.get("list")?,
                        address: row.get("address")?,
                        name: row.get("name")?,
                        accepted: row.get("accepted")?,
                    },
                    pk,
                ))
            },
        )?;
        drop(stmt);

        trace!("add_candidate_subscription {:?}.", &val);
        // table entry might be modified by triggers, so don't rely on RETURNING value.
        self.candidate_subscription(val.pk())
    }

    /// Fetch subscription candidate by primary key.
    pub fn candidate_subscription(&self, pk: i64) -> Result<DbVal<ListCandidateSubscription>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM candidate_subscription WHERE pk = ?;")?;
        let val = stmt
            .query_row(rusqlite::params![&pk], |row| {
                let _pk: i64 = row.get("pk")?;
                debug_assert_eq!(pk, _pk);
                Ok(DbVal(
                    ListCandidateSubscription {
                        pk,
                        list: row.get("list")?,
                        address: row.get("address")?,
                        name: row.get("name")?,
                        accepted: row.get("accepted")?,
                    },
                    pk,
                ))
            })
            .map_err(|err| {
                if matches!(err, rusqlite::Error::QueryReturnedNoRows) {
                    Error::from(format!(
                        "{err} {}",
                        Error::NotFound("Candidate subscription with this pk not found!")
                    ))
                } else {
                    err.into()
                }
            })?;

        Ok(val)
    }

    /// Accept subscription candidate.
    pub fn accept_candidate_subscription(&self, pk: i64) -> Result<DbVal<ListSubscription>> {
        let val = self.connection.query_row(
            "INSERT INTO subscription(list, address, name, enabled, digest, verified, \
             hide_address, receive_duplicates, receive_own_posts, receive_confirmation) SELECT \
             list, address, name, 1, 0, 0, 0, 1, 1, 0 FROM candidate_subscription WHERE pk = ? \
             RETURNING *;",
            rusqlite::params![&pk],
            |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    ListSubscription {
                        pk,
                        list: row.get("list")?,
                        address: row.get("address")?,
                        account: row.get("account")?,
                        name: row.get("name")?,
                        digest: row.get("digest")?,
                        enabled: row.get("enabled")?,
                        verified: row.get("verified")?,
                        hide_address: row.get("hide_address")?,
                        receive_duplicates: row.get("receive_duplicates")?,
                        receive_own_posts: row.get("receive_own_posts")?,
                        receive_confirmation: row.get("receive_confirmation")?,
                    },
                    pk,
                ))
            },
        )?;

        trace!("accept_candidate_subscription {:?}.", &val);
        // table entry might be modified by triggers, so don't rely on RETURNING value.
        let ret = self.list_subscription(val.list, val.pk())?;

        // assert that [ref:accept_candidate] trigger works.
        debug_assert_eq!(Some(ret.pk), self.candidate_subscription(pk)?.accepted);
        Ok(ret)
    }

    /// Remove a subscription by their address.
    pub fn remove_subscription(&self, list_pk: i64, address: &str) -> Result<()> {
        self.connection
            .query_row(
                "DELETE FROM subscription WHERE list = ? AND address = ? RETURNING *;",
                rusqlite::params![&list_pk, &address],
                |_| Ok(()),
            )
            .map_err(|err| {
                if matches!(err, rusqlite::Error::QueryReturnedNoRows) {
                    Error::from(format!(
                        "{err} {}",
                        Error::NotFound("list or list owner not found!")
                    ))
                } else {
                    err.into()
                }
            })?;

        Ok(())
    }

    /// Update a mailing list subscription.
    pub fn update_subscription(&self, change_set: ListSubscriptionChangeset) -> Result<()> {
        let pk = self
            .list_subscription_by_address(change_set.list, &change_set.address)?
            .pk;
        if matches!(
            change_set,
            ListSubscriptionChangeset {
                list: _,
                address: _,
                account: None,
                name: None,
                digest: None,
                verified: None,
                hide_address: None,
                receive_duplicates: None,
                receive_own_posts: None,
                receive_confirmation: None,
                enabled: None,
            }
        ) {
            return Ok(());
        }

        let ListSubscriptionChangeset {
            list,
            address: _,
            name,
            account,
            digest,
            enabled,
            verified,
            hide_address,
            receive_duplicates,
            receive_own_posts,
            receive_confirmation,
        } = change_set;
        let tx = self.savepoint(Some(stringify!(update_subscription)))?;

        macro_rules! update {
            ($field:tt) => {{
                if let Some($field) = $field {
                    tx.connection.execute(
                        concat!(
                            "UPDATE subscription SET ",
                            stringify!($field),
                            " = ? WHERE list = ? AND pk = ?;"
                        ),
                        rusqlite::params![&$field, &list, &pk],
                    )?;
                }
            }};
        }
        update!(name);
        update!(account);
        update!(digest);
        update!(enabled);
        update!(verified);
        update!(hide_address);
        update!(receive_duplicates);
        update!(receive_own_posts);
        update!(receive_confirmation);

        tx.commit()?;
        Ok(())
    }

    /// Fetch account by pk.
    pub fn account(&self, pk: i64) -> Result<Option<DbVal<Account>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM account WHERE pk = ?;")?;

        let ret = stmt
            .query_row(rusqlite::params![&pk], |row| {
                let _pk: i64 = row.get("pk")?;
                debug_assert_eq!(pk, _pk);
                Ok(DbVal(
                    Account {
                        pk,
                        name: row.get("name")?,
                        address: row.get("address")?,
                        public_key: row.get("public_key")?,
                        password: row.get("password")?,
                        enabled: row.get("enabled")?,
                    },
                    pk,
                ))
            })
            .optional()?;
        Ok(ret)
    }

    /// Fetch account by address.
    pub fn account_by_address(&self, address: &str) -> Result<Option<DbVal<Account>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM account WHERE address = ?;")?;

        let ret = stmt
            .query_row(rusqlite::params![&address], |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    Account {
                        pk,
                        name: row.get("name")?,
                        address: row.get("address")?,
                        public_key: row.get("public_key")?,
                        password: row.get("password")?,
                        enabled: row.get("enabled")?,
                    },
                    pk,
                ))
            })
            .optional()?;
        Ok(ret)
    }

    /// Fetch all subscriptions of an account by primary key.
    pub fn account_subscriptions(&self, pk: i64) -> Result<Vec<DbVal<ListSubscription>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM subscription WHERE account = ?;")?;
        let list_iter = stmt.query_map([&pk], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                ListSubscription {
                    pk: row.get("pk")?,
                    list: row.get("list")?,
                    address: row.get("address")?,
                    account: row.get("account")?,
                    name: row.get("name")?,
                    digest: row.get("digest")?,
                    enabled: row.get("enabled")?,
                    verified: row.get("verified")?,
                    hide_address: row.get("hide_address")?,
                    receive_duplicates: row.get("receive_duplicates")?,
                    receive_own_posts: row.get("receive_own_posts")?,
                    receive_confirmation: row.get("receive_confirmation")?,
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

    /// Fetch all accounts.
    pub fn accounts(&self) -> Result<Vec<DbVal<Account>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM account ORDER BY pk ASC;")?;
        let list_iter = stmt.query_map([], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                Account {
                    pk,
                    name: row.get("name")?,
                    address: row.get("address")?,
                    public_key: row.get("public_key")?,
                    password: row.get("password")?,
                    enabled: row.get("enabled")?,
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

    /// Add account.
    pub fn add_account(&self, new_val: Account) -> Result<DbVal<Account>> {
        let mut stmt = self
            .connection
            .prepare(
                "INSERT INTO account(name, address, public_key, password, enabled) VALUES(?, ?, \
                 ?, ?, ?) RETURNING *;",
            )
            .unwrap();
        let ret = stmt.query_row(
            rusqlite::params![
                &new_val.name,
                &new_val.address,
                &new_val.public_key,
                &new_val.password,
                &new_val.enabled,
            ],
            |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    Account {
                        pk,
                        name: row.get("name")?,
                        address: row.get("address")?,
                        public_key: row.get("public_key")?,
                        password: row.get("password")?,
                        enabled: row.get("enabled")?,
                    },
                    pk,
                ))
            },
        )?;

        trace!("add_account {:?}.", &ret);
        Ok(ret)
    }

    /// Remove an account by their address.
    pub fn remove_account(&self, address: &str) -> Result<()> {
        self.connection
            .query_row(
                "DELETE FROM account WHERE address = ? RETURNING *;",
                rusqlite::params![&address],
                |_| Ok(()),
            )
            .map_err(|err| {
                if matches!(err, rusqlite::Error::QueryReturnedNoRows) {
                    Error::from(format!("{err} {}", Error::NotFound("account not found!")))
                } else {
                    err.into()
                }
            })?;

        Ok(())
    }

    /// Update an account.
    pub fn update_account(&self, change_set: AccountChangeset) -> Result<()> {
        let Some(acc) = self.account_by_address(&change_set.address)? else {
            return Err(Error::NotFound("account with this address not found!"));
        };
        let pk = acc.pk;
        if matches!(
            change_set,
            AccountChangeset {
                address: _,
                name: None,
                public_key: None,
                password: None,
                enabled: None,
            }
        ) {
            return Ok(());
        }

        let AccountChangeset {
            address: _,
            name,
            public_key,
            password,
            enabled,
        } = change_set;
        let tx = self.savepoint(Some(stringify!(update_account)))?;

        macro_rules! update {
            ($field:tt) => {{
                if let Some($field) = $field {
                    tx.connection.execute(
                        concat!(
                            "UPDATE account SET ",
                            stringify!($field),
                            " = ? WHERE pk = ?;"
                        ),
                        rusqlite::params![&$field, &pk],
                    )?;
                }
            }};
        }
        update!(name);
        update!(public_key);
        update!(password);
        update!(enabled);

        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn test_subscription_ops() {
        use tempfile::TempDir;

        let tmp_dir = TempDir::new().unwrap();
        let db_path = tmp_dir.path().join("mpot.db");
        let config = Configuration {
            send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
            db_path,
            data_path: tmp_dir.path().to_path_buf(),
            administrators: vec![],
        };

        let db = Connection::open_or_create_db(config).unwrap().trusted();
        let list = db
            .create_list(MailingList {
                pk: -1,
                name: "foobar chat".into(),
                id: "foo-chat".into(),
                address: "foo-chat@example.com".into(),
                topics: vec![],
                description: None,
                archive_url: None,
            })
            .unwrap();
        let secondary_list = db
            .create_list(MailingList {
                pk: -1,
                name: "foobar chat2".into(),
                id: "foo-chat2".into(),
                address: "foo-chat2@example.com".into(),
                topics: vec![],
                description: None,
                archive_url: None,
            })
            .unwrap();
        for i in 0..4 {
            let sub = db
                .add_subscription(
                    list.pk(),
                    ListSubscription {
                        pk: -1,
                        list: list.pk(),
                        address: format!("{i}@example.com"),
                        account: None,
                        name: Some(format!("User{i}")),
                        digest: false,
                        hide_address: false,
                        receive_duplicates: false,
                        receive_own_posts: false,
                        receive_confirmation: false,
                        enabled: true,
                        verified: false,
                    },
                )
                .unwrap();
            assert_eq!(db.list_subscription(list.pk(), sub.pk()).unwrap(), sub);
            assert_eq!(
                db.list_subscription_by_address(list.pk(), &sub.address)
                    .unwrap(),
                sub
            );
        }

        assert_eq!(db.accounts().unwrap(), vec![]);
        assert_eq!(
            db.remove_subscription(list.pk(), "nonexistent@example.com")
                .map_err(|err| err.to_string())
                .unwrap_err(),
            NotFound("list or list owner not found!").to_string()
        );

        let cand = db
            .add_candidate_subscription(
                list.pk(),
                ListSubscription {
                    pk: -1,
                    list: list.pk(),
                    address: "4@example.com".into(),
                    account: None,
                    name: Some("User4".into()),
                    digest: false,
                    hide_address: false,
                    receive_duplicates: false,
                    receive_own_posts: false,
                    receive_confirmation: false,
                    enabled: true,
                    verified: false,
                },
            )
            .unwrap();
        let accepted = db.accept_candidate_subscription(cand.pk()).unwrap();

        assert_eq!(db.account(5).unwrap(), None);
        assert_eq!(
            db.remove_account("4@example.com")
                .map_err(|err| err.to_string())
                .unwrap_err(),
            NotFound("account not found!").to_string()
        );

        let acc = db
            .add_account(Account {
                pk: -1,
                name: accepted.name.clone(),
                address: accepted.address.clone(),
                public_key: None,
                password: String::new(),
                enabled: true,
            })
            .unwrap();

        // Test [ref:add_account] SQL trigger (see schema.sql)
        assert_eq!(
            db.list_subscription(list.pk(), accepted.pk())
                .unwrap()
                .account,
            Some(acc.pk())
        );
        // Test [ref:add_account_to_subscription] SQL trigger (see schema.sql)
        let sub = db
            .add_subscription(
                secondary_list.pk(),
                ListSubscription {
                    pk: -1,
                    list: secondary_list.pk(),
                    address: "4@example.com".into(),
                    account: None,
                    name: Some("User4".into()),
                    digest: false,
                    hide_address: false,
                    receive_duplicates: false,
                    receive_own_posts: false,
                    receive_confirmation: false,
                    enabled: true,
                    verified: true,
                },
            )
            .unwrap();
        assert_eq!(sub.account, Some(acc.pk()));
        // Test [ref:verify_subscription_email] SQL trigger (see schema.sql)
        assert!(!sub.verified);

        assert_eq!(db.accounts().unwrap(), vec![acc.clone()]);

        assert_eq!(
            db.update_account(AccountChangeset {
                address: "nonexistent@example.com".into(),
                ..AccountChangeset::default()
            })
            .map_err(|err| err.to_string())
            .unwrap_err(),
            NotFound("account with this address not found!").to_string()
        );
        assert_eq!(
            db.update_account(AccountChangeset {
                address: acc.address.clone(),
                ..AccountChangeset::default()
            })
            .map_err(|err| err.to_string()),
            Ok(())
        );
        assert_eq!(
            db.update_account(AccountChangeset {
                address: acc.address.clone(),
                enabled: Some(Some(false)),
                ..AccountChangeset::default()
            })
            .map_err(|err| err.to_string()),
            Ok(())
        );
        assert!(!db.account(acc.pk()).unwrap().unwrap().enabled);
        assert_eq!(
            db.remove_account("4@example.com")
                .map_err(|err| err.to_string()),
            Ok(())
        );
        assert_eq!(db.accounts().unwrap(), vec![]);
    }
}
