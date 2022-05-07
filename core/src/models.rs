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
pub mod changesets;

use melib::email::Address;

pub struct DbVal<T>(pub T, pub i64);

impl<T> DbVal<T> {
    #[inline(always)]
    pub fn pk(&self) -> i64 {
        self.1
    }
}

impl<T> std::ops::Deref for DbVal<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::fmt::Display for DbVal<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{}", self.0)
    }
}

impl<T> std::fmt::Debug for DbVal<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self.0)
    }
}

impl<T> serde::Serialize for DbVal<T>
where
    T: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<T> std::cmp::PartialEq for DbVal<T>
where
    T: std::cmp::PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
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

impl MailingList {
    pub fn list_id(&self) -> String {
        format!("\"{}\" <{}>", self.name, self.address)
    }

    pub fn list_post(&self) -> Option<String> {
        Some(format!("<mailto:{}>", self.address))
    }

    pub fn list_unsubscribe(&self) -> Option<String> {
        let p = self.address.split('@').collect::<Vec<&str>>();
        Some(format!(
            "<mailto:{}-request@{}?subject=unsubscribe>",
            p[0], p[1]
        ))
    }

    pub fn list_archive(&self) -> Option<String> {
        self.archive_url.as_ref().map(|url| format!("<{}>", url))
    }

    pub fn list_address(&self) -> Address {
        Address::new(Some(self.name.clone()), self.address.clone())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListMembership {
    pub pk: i64,
    pub list: i64,
    pub address: String,
    pub name: Option<String>,
    pub digest: bool,
    pub hide_address: bool,
    pub receive_duplicates: bool,
    pub receive_own_posts: bool,
    pub receive_confirmation: bool,
    pub enabled: bool,
}

impl std::fmt::Display for ListMembership {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            fmt,
            "{} [digest: {}, hide_address: {} {}]",
            self.into_address(),
            self.digest,
            self.hide_address,
            if self.enabled {
                "enabled"
            } else {
                "not enabled"
            },
        )
    }
}

impl ListMembership {
    pub fn into_address(&self) -> Address {
        Address::new(self.name.clone(), self.address.clone())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostPolicy {
    pub pk: i64,
    pub list: i64,
    pub announce_only: bool,
    pub subscriber_only: bool,
    pub approval_needed: bool,
}

impl std::fmt::Display for PostPolicy {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListOwner {
    pub pk: i64,
    pub list: i64,
    pub address: String,
    pub name: Option<String>,
}

impl std::fmt::Display for ListOwner {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "[#{} {}] {}", self.pk, self.list, self.into_address())
    }
}

impl From<ListOwner> for ListMembership {
    fn from(val: ListOwner) -> ListMembership {
        ListMembership {
            pk: 0,
            list: val.list,
            address: val.address,
            name: val.name,
            digest: false,
            hide_address: false,
            receive_duplicates: true,
            receive_own_posts: false,
            receive_confirmation: true,
            enabled: true,
        }
    }
}

impl ListOwner {
    pub fn into_address(&self) -> Address {
        Address::new(self.name.clone(), self.address.clone())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ListRequest {
    Subscribe,
    Unsubscribe,
    Other(String),
}

impl std::fmt::Display for ListRequest {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewListPost<'s> {
    pub list: i64,
    pub address: &'s str,
    pub message_id: &'s str,
    pub message: &'s [u8],
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Post {
    pub pk: i64,
    pub list: i64,
    pub address: String,
    pub message_id: String,
    pub message: Vec<u8>,
    pub timestamp: u64,
    pub datetime: String,
}

impl std::fmt::Display for Post {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}
