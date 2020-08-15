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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MailingListChangeset {
    pub pk: i64,
    pub name: Option<String>,
    pub id: Option<String>,
    pub address: Option<String>,
    pub description: Option<Option<String>>,
    pub archive_url: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListMembershipChangeset {
    pub list: i64,
    pub address: String,
    pub name: Option<Option<String>>,
    pub digest: Option<bool>,
    pub hide_address: Option<bool>,
    pub receive_duplicates: Option<bool>,
    pub receive_own_posts: Option<bool>,
    pub receive_confirmation: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostPolicyChangeset {
    pub pk: i64,
    pub list: i64,
    pub announce_only: Option<bool>,
    pub subscriber_only: Option<bool>,
    pub approval_needed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListOwnerChangeset {
    pub pk: i64,
    pub list: i64,
    pub address: Option<String>,
    pub name: Option<Option<String>>,
}

impl std::fmt::Display for MailingListChangeset {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

impl std::fmt::Display for ListMembershipChangeset {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}
impl std::fmt::Display for PostPolicyChangeset {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}
impl std::fmt::Display for ListOwnerChangeset {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}
