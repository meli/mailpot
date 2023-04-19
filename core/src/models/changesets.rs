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

//! Changeset structs: update specific struct fields.

macro_rules! impl_display {
    ($t:ty) => {
        impl std::fmt::Display for $t {
            fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(fmt, "{:?}", self)
            }
        }
    };
}

/// Changeset struct for [`Mailinglist`](super::MailingList).
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct MailingListChangeset {
    /// Database primary key.
    pub pk: i64,
    /// Optional new value.
    pub name: Option<String>,
    /// Optional new value.
    pub id: Option<String>,
    /// Optional new value.
    pub address: Option<String>,
    /// Optional new value.
    pub description: Option<Option<String>>,
    /// Optional new value.
    pub archive_url: Option<Option<String>>,
    /// Optional new value.
    pub owner_local_part: Option<Option<String>>,
    /// Optional new value.
    pub request_local_part: Option<Option<String>>,
    /// Optional new value.
    pub verify: Option<bool>,
    /// Optional new value.
    pub hidden: Option<bool>,
    /// Optional new value.
    pub enabled: Option<bool>,
}

impl_display!(MailingListChangeset);

/// Changeset struct for [`ListSubscription`](super::ListSubscription).
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct ListSubscriptionChangeset {
    /// Mailing list foreign key (See [`MailingList`](super::MailingList)).
    pub list: i64,
    /// Subscription e-mail address.
    pub address: String,
    /// Optional new value.
    pub account: Option<Option<i64>>,
    /// Optional new value.
    pub name: Option<Option<String>>,
    /// Optional new value.
    pub digest: Option<bool>,
    /// Optional new value.
    pub enabled: Option<bool>,
    /// Optional new value.
    pub verified: Option<bool>,
    /// Optional new value.
    pub hide_address: Option<bool>,
    /// Optional new value.
    pub receive_duplicates: Option<bool>,
    /// Optional new value.
    pub receive_own_posts: Option<bool>,
    /// Optional new value.
    pub receive_confirmation: Option<bool>,
}

impl_display!(ListSubscriptionChangeset);

/// Changeset struct for [`PostPolicy`](super::PostPolicy).
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct PostPolicyChangeset {
    /// Database primary key.
    pub pk: i64,
    /// Mailing list foreign key (See [`MailingList`](super::MailingList)).
    pub list: i64,
    /// Optional new value.
    pub announce_only: Option<bool>,
    /// Optional new value.
    pub subscription_only: Option<bool>,
    /// Optional new value.
    pub approval_needed: Option<bool>,
}

impl_display!(PostPolicyChangeset);

/// Changeset struct for [`ListOwner`](super::ListOwner).
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct ListOwnerChangeset {
    /// Database primary key.
    pub pk: i64,
    /// Mailing list foreign key (See [`MailingList`](super::MailingList)).
    pub list: i64,
    /// Optional new value.
    pub address: Option<String>,
    /// Optional new value.
    pub name: Option<Option<String>>,
}

impl_display!(ListOwnerChangeset);

/// Changeset struct for [`Account`](super::Account).
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct AccountChangeset {
    /// Account e-mail address.
    pub address: String,
    /// Optional new value.
    pub name: Option<Option<String>>,
    /// Optional new value.
    pub public_key: Option<Option<String>>,
    /// Optional new value.
    pub password: Option<String>,
    /// Optional new value.
    pub enabled: Option<Option<bool>>,
}

impl_display!(AccountChangeset);
