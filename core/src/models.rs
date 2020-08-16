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
use schema::*;

#[derive(Debug, Clone, Insertable, Queryable, Deserialize, Serialize)]
#[table_name = "mailing_lists"]
pub struct MailingList {
    pub pk: i32,
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

#[derive(Debug, Clone, Insertable, Queryable, Deserialize, Serialize)]
#[table_name = "membership"]
pub struct ListMembership {
    pub list: i32,
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

#[derive(Debug, Clone, Insertable, Queryable, Deserialize, Serialize)]
#[table_name = "post_policy"]
pub struct PostPolicy {
    pub pk: i32,
    pub list: i32,
    pub announce_only: bool,
    pub subscriber_only: bool,
    pub approval_needed: bool,
}

impl std::fmt::Display for PostPolicy {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

#[derive(Debug, Clone, Insertable, Queryable, Deserialize, Serialize)]
#[table_name = "list_owner"]
pub struct ListOwner {
    pub pk: i32,
    pub list: i32,
    pub address: String,
    pub name: Option<String>,
}

impl std::fmt::Display for ListOwner {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(ref name) = self.name {
            write!(
                fmt,
                "[#{} {}] \"{}\" <{}>",
                self.pk, self.list, name, self.address
            )
        } else {
            write!(fmt, "[#{} {}] {}", self.pk, self.list, self.address)
        }
    }
}

impl From<ListOwner> for ListMembership {
    fn from(val: ListOwner) -> ListMembership {
        ListMembership {
            list: val.list,
            address: val.address,
            name: val.name,
            digest: false,
            hide_address: false,
            receive_duplicates: true,
            receive_own_posts: false,
            receive_confirmation: true,
        }
    }
}
