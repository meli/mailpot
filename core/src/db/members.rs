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

impl Database {
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
                    hide_address: row.get("hide_address")?,
                    receive_duplicates: row.get("receive_duplicates")?,
                    receive_own_posts: row.get("receive_own_posts")?,
                    receive_confirmation: row.get("receive_confirmation")?,
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

    pub fn add_member(
        &self,
        list_pk: i64,
        mut new_val: ListMembership,
    ) -> Result<DbVal<ListMembership>> {
        new_val.list = list_pk;
        let mut stmt = self
            .connection
            .prepare("INSERT INTO membership(list, address, name, enabled, digest, hide_address, receive_duplicates, receive_own_posts, receive_confirmation) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *;")?;
        let ret = stmt.query_row(
            rusqlite::params![
                &new_val.list,
                &new_val.address,
                &new_val.name,
                &new_val.enabled,
                &new_val.digest,
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
                        hide_address: row.get("hide_address")?,
                        receive_duplicates: row.get("receive_duplicates")?,
                        receive_own_posts: row.get("receive_own_posts")?,
                        receive_confirmation: row.get("receive_confirmation")?,
                        enabled: row.get("enabled")?,
                    },
                    pk,
                ))
            },
        )?;

        trace!("add_member {:?}.", &ret);
        Ok(ret)
    }

    pub fn add_candidate_member(&self, list_pk: i64, mut new_val: ListMembership) -> Result<i64> {
        new_val.list = list_pk;
        let mut stmt = self
            .connection
            .prepare("INSERT INTO candidate_membership(list, address, name, accepted) VALUES(?, ?, ?, ?) RETURNING pk;")?;
        let ret = stmt.query_row(
            rusqlite::params![&new_val.list, &new_val.address, &new_val.name, &false,],
            |row| {
                let pk: i64 = row.get("pk")?;
                Ok(pk)
            },
        )?;

        trace!("add_candidate_member {:?}.", &ret);
        Ok(ret)
    }

    pub fn accept_candidate_member(&mut self, pk: i64) -> Result<DbVal<ListMembership>> {
        let tx = self.connection.transaction()?;
        let mut stmt = tx
            .prepare("INSERT INTO membership(list, address, name, enabled, digest, hide_address, receive_duplicates, receive_own_posts, receive_confirmation) FROM (SELECT list, address, name FROM candidate_membership WHERE pk = ?) RETURNING *;")?;
        let ret = stmt.query_row(rusqlite::params![&pk], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                ListMembership {
                    pk,
                    list: row.get("list")?,
                    address: row.get("address")?,
                    name: row.get("name")?,
                    digest: row.get("digest")?,
                    hide_address: row.get("hide_address")?,
                    receive_duplicates: row.get("receive_duplicates")?,
                    receive_own_posts: row.get("receive_own_posts")?,
                    receive_confirmation: row.get("receive_confirmation")?,
                    enabled: row.get("enabled")?,
                },
                pk,
            ))
        })?;
        drop(stmt);
        tx.execute(
            "UPDATE candidate_membership SET accepted = ? WHERE pk = ?;",
            [&pk],
        )?;
        tx.commit()?;

        trace!("accept_candidate_member {:?}.", &ret);
        Ok(ret)
    }

    pub fn remove_member(&self, list_pk: i64, address: &str) -> Result<()> {
        self.connection.execute(
            "DELETE FROM membership WHERE list_pk = ? AND address = ?;",
            rusqlite::params![&list_pk, &address],
        )?;

        Ok(())
    }

    pub fn update_member(&self, _change_set: ListMembershipChangeset) -> Result<()> {
        /*
        diesel::update(membership::table)
            .set(&set)
            .execute(&self.connection)?;
        */
        Ok(())
    }
}
