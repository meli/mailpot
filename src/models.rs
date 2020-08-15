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
use rusqlite::Row;
use std::convert::TryFrom;

#[derive(Debug)]
pub struct MailingList {
    pub pk: i64,
    pub name: String,
    pub id: String,
    pub address: String,
    pub description: Option<String>,
    pub archive_url: Option<String>,
}

impl std::fmt::Display for MailingList {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(description) = self.description.as_ref() {
            write!(
                fmt,
                "[#{} {}] {} <{}>: {}",
                self.pk, self.id, self.name, self.address, description
            )
        } else {
            write!(
                fmt,
                "[#{} {}] {} <{}>",
                self.pk, self.id, self.name, self.address
            )
        }
    }
}

impl TryFrom<&'_ Row<'_>> for MailingList {
    type Error = rusqlite::Error;
    fn try_from(row: &'_ Row<'_>) -> std::result::Result<MailingList, rusqlite::Error> {
        Ok(MailingList {
            pk: row.get("pk")?,
            name: row.get("name")?,
            id: row.get("id")?,
            address: row.get("address")?,
            description: row.get("description")?,
            archive_url: row.get("archive_url")?,
        })
    }
}

impl MailingList {
    pub fn list_id(&self) -> String {
        format!("\"{}\" <{}>", self.name, self.address)
    }

    pub fn list_post(&self) -> Option<String> {
        Some(format!("<mailto:{}>", self.address))
    }

    pub fn list_unsubscribe(&self) -> Option<String> {
        let p = self.address.split("@").collect::<Vec<&str>>();
        Some(format!(
            "<mailto:{}-request@{}?subject=unsubscribe>",
            p[0], p[1]
        ))
    }

    pub fn list_archive(&self) -> Option<String> {
        self.archive_url.as_ref().map(|url| format!("<{}>", url))
    }
}

#[derive(Debug)]
pub struct ListMembership {
    pub list: i64,
    pub address: String,
    pub name: Option<String>,
    pub digest: bool,
    pub hide_address: bool,
    pub receive_duplicates: bool,
    pub receive_own_posts: bool,
    pub receive_confirmation: bool,
}

impl std::fmt::Display for ListMembership {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(name) = self.name.as_ref() {
            write!(
                fmt,
                "{} <{}> [digest: {}, hide_address: {}]",
                name, self.address, self.digest, self.hide_address
            )
        } else {
            write!(
                fmt,
                "{} [digest: {}, hide_address: {}]",
                self.address, self.digest, self.hide_address
            )
        }
    }
}

impl TryFrom<&'_ Row<'_>> for ListMembership {
    type Error = rusqlite::Error;
    fn try_from(row: &'_ Row<'_>) -> std::result::Result<ListMembership, rusqlite::Error> {
        Ok(ListMembership {
            list: row.get("list")?,
            address: row.get("address")?,
            name: row.get("name")?,
            digest: row.get("digest")?,
            hide_address: row.get("hide_address")?,
            receive_duplicates: row.get("receive_duplicates")?,
            receive_own_posts: row.get("receive_own_posts")?,
            receive_confirmation: row.get("receive_confirmation")?,
        })
    }
}

impl ListMembership {
    pub fn into_address(&self) -> melib::email::Address {
        use melib::email::Address;
        use melib::email::StrBuilder;
        use melib::MailboxAddress;
        if let Some(name) = self.name.as_ref() {
            melib::make_address!(name, self.address)
        } else {
            melib::make_address!("", self.address)
        }
    }
}
