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

impl Connection {
    /// Fetch all members of a mailing list.
    pub fn list_members(&self, pk: i64) -> Result<Vec<DbVal<ListMembership>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM membership WHERE list = ?;")?;
        let list_iter = stmt.query_map([&pk], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                ListMembership {
                    pk: row.get("pk")?,
                    list: row.get("list")?,
                    address: row.get("address")?,
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

    /// Fetch mailing list member.
    pub fn list_member(&self, list_pk: i64, pk: i64) -> Result<DbVal<ListMembership>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM membership WHERE list = ? AND pk = ?;")?;

        let ret = stmt.query_row([&list_pk, &pk], |row| {
            let _pk: i64 = row.get("pk")?;
            debug_assert_eq!(pk, _pk);
            Ok(DbVal(
                ListMembership {
                    pk,
                    list: row.get("list")?,
                    address: row.get("address")?,
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

    /// Fetch mailing list member by their address.
    pub fn list_member_by_address(
        &self,
        list_pk: i64,
        address: &str,
    ) -> Result<DbVal<ListMembership>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM membership WHERE list = ? AND address = ?;")?;

        let ret = stmt.query_row(rusqlite::params![&list_pk, &address], |row| {
            let pk = row.get("pk")?;
            let address_ = row.get("address")?;
            debug_assert_eq!(address, &address_);
            Ok(DbVal(
                ListMembership {
                    pk,
                    list: row.get("list")?,
                    address: address_,
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

    /// Add member to mailing list.
    pub fn add_member(
        &self,
        list_pk: i64,
        mut new_val: ListMembership,
    ) -> Result<DbVal<ListMembership>> {
        new_val.list = list_pk;
        let mut stmt = self
            .connection
            .prepare("INSERT INTO membership(list, address, name, enabled, digest, verified, hide_address, receive_duplicates, receive_own_posts, receive_confirmation) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *;").unwrap();
        let ret = stmt.query_row(
            rusqlite::params![
                &new_val.list,
                &new_val.address,
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
                    ListMembership {
                        pk,
                        list: row.get("list")?,
                        address: row.get("address")?,
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

        trace!("add_member {:?}.", &ret);
        Ok(ret)
    }

    /// Create membership candidate.
    pub fn add_candidate_member(
        &mut self,
        list_pk: i64,
        mut new_val: ListMembership,
    ) -> Result<i64> {
        new_val.list = list_pk;
        let mut stmt = self
            .connection
            .prepare("INSERT INTO candidate_membership(list, address, name, accepted) VALUES(?, ?, ?, ?) RETURNING pk;")?;
        let ret = stmt.query_row(
            rusqlite::params![&new_val.list, &new_val.address, &new_val.name, None::<i64>,],
            |row| {
                let pk: i64 = row.get("pk")?;
                Ok(pk)
            },
        )?;
        drop(stmt);

        trace!("add_candidate_member {:?}.", &ret);
        self.accept_candidate_member(ret)?;
        // [ref:FIXME]: add approval required option for subscriptions.
        Ok(ret)
    }

    /// Accept membership candidate.
    pub fn accept_candidate_member(&mut self, pk: i64) -> Result<DbVal<ListMembership>> {
        let tx = self.connection.transaction()?;
        let mut stmt = tx
            .prepare("INSERT INTO membership(list, address, name, enabled, digest, verified, hide_address, receive_duplicates, receive_own_posts, receive_confirmation) SELECT list, address, name, 1, 0, 0, 0, 1, 1, 0 FROM candidate_membership WHERE pk = ? RETURNING *;")?;
        let ret = stmt.query_row(rusqlite::params![&pk], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                ListMembership {
                    pk,
                    list: row.get("list")?,
                    address: row.get("address")?,
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
        drop(stmt);
        tx.execute(
            "UPDATE candidate_membership SET accepted = ? WHERE pk = ?;",
            [&ret.pk, &pk],
        )?;
        tx.commit()?;

        trace!("accept_candidate_member {:?}.", &ret);
        Ok(ret)
    }

    /// Remove a member by their address.
    pub fn remove_membership(&self, list_pk: i64, address: &str) -> Result<()> {
        self.connection
            .query_row(
                "DELETE FROM membership WHERE list_pk = ? AND address = ? RETURNING *;",
                rusqlite::params![&list_pk, &address],
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

    /// Update a mailing list membership.
    pub fn update_member(&mut self, change_set: ListMembershipChangeset) -> Result<()> {
        let pk = self
            .list_member_by_address(change_set.list, &change_set.address)?
            .pk;
        if matches!(
            change_set,
            ListMembershipChangeset {
                list: _,
                address: _,
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

        let ListMembershipChangeset {
            list,
            address: _,
            name,
            digest,
            enabled,
            verified,
            hide_address,
            receive_duplicates,
            receive_own_posts,
            receive_confirmation,
        } = change_set;
        let tx = self.connection.transaction()?;

        macro_rules! update {
            ($field:tt) => {{
                if let Some($field) = $field {
                    tx.execute(
                        concat!(
                            "UPDATE membership SET ",
                            stringify!($field),
                            " = ? WHERE list = ? AND pk = ?;"
                        ),
                        rusqlite::params![&$field, &list, &pk],
                    )?;
                }
            }};
        }
        update!(name);
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
    pub fn account_subscriptions(&self, pk: i64) -> Result<Vec<DbVal<ListMembership>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM membership WHERE account = ?;")?;
        let list_iter = stmt.query_map([&pk], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                ListMembership {
                    pk: row.get("pk")?,
                    list: row.get("list")?,
                    address: row.get("address")?,
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
            .prepare("INSERT INTO account(name, address, public_key, password, enabled) VALUES(?, ?, ?, ?, ?) RETURNING *;").unwrap();
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
                    Error::from(err).chain_err(|| NotFound("account not found!"))
                } else {
                    err.into()
                }
            })?;

        Ok(())
    }

    /// Update an account.
    pub fn update_account(&mut self, change_set: AccountChangeset) -> Result<()> {
        let Some(acc) = self.account_by_address(&change_set.address)? else {
            return Err(NotFound("account with this address not found!").into());
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
        let tx = self.connection.transaction()?;

        macro_rules! update {
            ($field:tt) => {{
                if let Some($field) = $field {
                    tx.execute(
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
