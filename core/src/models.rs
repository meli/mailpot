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

//! Database models: [`MailingList`], [`ListOwner`], [`ListMembership`], [`PostPolicy`] and
//! [`Post`].

use super::*;
pub mod changesets;

use melib::email::Address;

/// A database entry and its primary key. Derefs to its inner type.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct DbVal<T>(pub T, #[serde(skip)] pub i64);

impl<T> DbVal<T> {
    /// Primary key.
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

/// A mailing list entry.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub struct MailingList {
    /// Database primary key.
    pub pk: i64,
    /// Mailing list name.
    pub name: String,
    /// Mailing list ID (what appears in the subject tag, e.g. `[mailing-list] New post!`).
    pub id: String,
    /// Mailing list e-mail address.
    pub address: String,
    /// Mailing list description.
    pub description: Option<String>,
    /// Mailing list archive URL.
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
    /// Mailing list display name (e.g. `list name <list_address@example.com>`).
    pub fn display_name(&self) -> String {
        format!("\"{}\" <{}>", self.name, self.address)
    }

    /// Value of `List-Post` header.
    ///
    /// See RFC2369 Section 3.4: <https://www.rfc-editor.org/rfc/rfc2369#section-3.4>
    pub fn post_header(&self) -> Option<String> {
        Some(format!("<mailto:{}>", self.address))
    }

    /// Value of `List-Unsubscribe` header.
    ///
    /// See RFC2369 Section 3.2: <https://www.rfc-editor.org/rfc/rfc2369#section-3.2>
    pub fn unsubscribe_header(&self) -> Option<String> {
        let p = self.address.split('@').collect::<Vec<&str>>();
        Some(format!(
            "<mailto:{}-request@{}?subject=subscribe>",
            p[0], p[1]
        ))
    }

    /// Value of `List-Archive` header.
    ///
    /// See RFC2369 Section 3.6: <https://www.rfc-editor.org/rfc/rfc2369#section-3.6>
    pub fn archive_header(&self) -> Option<String> {
        self.archive_url.as_ref().map(|url| format!("<{}>", url))
    }

    /// List address as a [`melib::Address`]
    pub fn address(&self) -> Address {
        Address::new(Some(self.name.clone()), self.address.clone())
    }

    /// List unsubscribe action as a [`MailtoAddress`](super::MailtoAddress).
    pub fn unsubscribe_mailto(&self) -> Option<MailtoAddress> {
        let p = self.address.split('@').collect::<Vec<&str>>();
        Some(MailtoAddress {
            address: format!("{}-request@{}", p[0], p[1]),
            subject: Some("unsubscribe".to_string()),
        })
    }

    /// List subscribe action as a [`MailtoAddress`](super::MailtoAddress).
    pub fn subscribe_mailto(&self) -> Option<MailtoAddress> {
        let p = self.address.split('@').collect::<Vec<&str>>();
        Some(MailtoAddress {
            address: format!("{}-request@{}", p[0], p[1]),
            subject: Some("subscribe".to_string()),
        })
    }

    /// List archive url value.
    pub fn archive_url(&self) -> Option<&str> {
        self.archive_url.as_deref()
    }
}

/// A mailing list membership entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListMembership {
    /// Database primary key.
    pub pk: i64,
    /// Mailing list foreign key (See [`MailingList`]).
    pub list: i64,
    /// Member's e-mail address.
    pub address: String,
    /// Member's name, optional.
    pub name: Option<String>,
    /// Whether member wishes to receive list posts as a periodical digest e-mail.
    pub digest: bool,
    /// Whether member wishes their e-mail address hidden from public view.
    pub hide_address: bool,
    /// Whether member wishes to receive mailing list post duplicates, i.e. posts addressed to them
    /// and the mailing list to which they are subscribed.
    pub receive_duplicates: bool,
    /// Whether member wishes to receive their own mailing list posts from the mailing list, as a
    /// confirmation.
    pub receive_own_posts: bool,
    /// Whether member wishes to receive a plain confirmation for their own mailing list posts.
    pub receive_confirmation: bool,
    /// Whether this membership is enabled.
    pub enabled: bool,
}

impl std::fmt::Display for ListMembership {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            fmt,
            "{} [digest: {}, hide_address: {} {}]",
            self.address(),
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
    /// Member address as a [`melib::Address`]
    pub fn address(&self) -> Address {
        Address::new(self.name.clone(), self.address.clone())
    }
}

/// A mailing list post policy entry.
///
/// Only one of the boolean flags must be set to true.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostPolicy {
    /// Database primary key.
    pub pk: i64,
    /// Mailing list foreign key (See [`MailingList`]).
    pub list: i64,
    /// Whether the policy is announce only (Only list owners can submit posts, and everyone will
    /// receive them).
    pub announce_only: bool,
    /// Whether the policy is "subscriber only" (Only list subscribers can post).
    pub subscriber_only: bool,
    /// Whether the policy is "approval needed" (Anyone can post, but approval from list owners is
    /// required if they are not subscribed).
    pub approval_needed: bool,
    /// Whether the policy is "no subscriptions" (Anyone can post, but approval from list owners is
    /// required. Subscriptions are not enabled).
    pub no_subscriptions: bool,
    /// Custom policy.
    pub custom: bool,
}

impl std::fmt::Display for PostPolicy {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/// A mailing list owner entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListOwner {
    /// Database primary key.
    pub pk: i64,
    /// Mailing list foreign key (See [`MailingList`]).
    pub list: i64,
    /// Mailing list owner e-mail address.
    pub address: String,
    /// Mailing list owner name, optional.
    pub name: Option<String>,
}

impl std::fmt::Display for ListOwner {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "[#{} {}] {}", self.pk, self.list, self.address())
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
    /// Owner address as a [`melib::Address`]
    pub fn address(&self) -> Address {
        Address::new(self.name.clone(), self.address.clone())
    }
}

/// A mailing list post entry.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Post {
    /// Database primary key.
    pub pk: i64,
    /// Mailing list foreign key (See [`MailingList`]).
    pub list: i64,
    /// `From` header address of post.
    pub address: String,
    /// `Message-ID` header value of post.
    pub message_id: String,
    /// Post as bytes.
    pub message: Vec<u8>,
    /// Unix timestamp of date.
    pub timestamp: u64,
    /// Datetime as string.
    pub datetime: String,
    /// Month-year as a `YYYY-mm` formatted string, for use in archives.
    pub month_year: String,
}

impl std::fmt::Display for Post {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}
