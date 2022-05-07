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
use melib::Address;
pub mod message_filters;

#[derive(Debug)]
pub enum PostAction {
    Hold,
    Accept,
    Reject { reason: String },
    Defer { reason: String },
}

#[derive(Debug)]
pub struct ListContext<'list> {
    pub list: &'list MailingList,
    pub list_owners: Vec<DbVal<ListOwner>>,
    pub memberships: &'list [DbVal<ListMembership>],
    pub policy: Option<DbVal<PostPolicy>>,
    pub scheduled_jobs: Vec<MailJob>,
}

///Post to be considered by the list's `PostFilter` stack.
pub struct Post {
    pub from: Address,
    pub bytes: Vec<u8>,
    pub to: Vec<Address>,
    pub action: PostAction,
}

impl core::fmt::Debug for Post {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        fmt.debug_struct("Post")
            .field("from", &self.from)
            .field("bytes", &format_args!("{} bytes", self.bytes.len()))
            .field("to", &self.to.as_slice())
            .field("action", &self.action)
            .finish()
    }
}

#[derive(Debug)]
pub enum MailJob {
    Send { recipients: Vec<Address> },
    Relay { recipients: Vec<Address> },
    Error { description: String },
    StoreDigest { recipients: Vec<Address> },
    ConfirmSubscription { recipient: Address },
    ConfirmUnsubscription { recipient: Address },
}
