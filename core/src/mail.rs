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

//! Types for processing new posts:
//! [`PostFilter`](crate::message_filters::PostFilter), [`ListContext`],
//! [`MailJob`] and [`PostAction`].

use std::collections::HashMap;

use log::trace;
use melib::{Address, MessageID};

use crate::{
    models::{ListOwner, ListSubscription, MailingList, PostPolicy, SubscriptionPolicy},
    DbVal,
};
/// Post action returned from a list's
/// [`PostFilter`](crate::message_filters::PostFilter) stack.
#[derive(Debug)]
pub enum PostAction {
    /// Add to `hold` queue.
    Hold,
    /// Accept to mailing list.
    Accept,
    /// Reject and send rejection response to submitter.
    Reject {
        /// Human readable reason for rejection.
        reason: String,
    },
    /// Add to `deferred` queue.
    Defer {
        /// Human readable reason for deferring.
        reason: String,
    },
}

/// List context passed to a list's
/// [`PostFilter`](crate::message_filters::PostFilter) stack.
#[derive(Debug)]
pub struct ListContext<'list> {
    /// Which mailing list a post was addressed to.
    pub list: &'list MailingList,
    /// The mailing list owners.
    pub list_owners: &'list [DbVal<ListOwner>],
    /// The mailing list subscriptions.
    pub subscriptions: &'list [DbVal<ListSubscription>],
    /// The mailing list post policy.
    pub post_policy: Option<DbVal<PostPolicy>>,
    /// The mailing list subscription policy.
    pub subscription_policy: Option<DbVal<SubscriptionPolicy>>,
    /// The scheduled jobs added by each filter in a list's
    /// [`PostFilter`](crate::message_filters::PostFilter) stack.
    pub scheduled_jobs: Vec<MailJob>,
    /// Saved settings for message filters, which process a
    /// received e-mail before taking a final decision/action.
    pub filter_settings: HashMap<String, DbVal<serde_json::Value>>,
}

/// Post to be considered by the list's
/// [`PostFilter`](crate::message_filters::PostFilter) stack.
pub struct PostEntry {
    /// `From` address of post.
    pub from: Address,
    /// Raw bytes of post.
    pub bytes: Vec<u8>,
    /// `To` addresses of post.
    pub to: Vec<Address>,
    /// Final action set by each filter in a list's
    /// [`PostFilter`](crate::message_filters::PostFilter) stack.
    pub action: PostAction,
    /// Post's Message-ID
    pub message_id: MessageID,
}

impl core::fmt::Debug for PostEntry {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        fmt.debug_struct(stringify!(PostEntry))
            .field("from", &self.from)
            .field("message_id", &self.message_id)
            .field("bytes", &format_args!("{} bytes", self.bytes.len()))
            .field("to", &self.to.as_slice())
            .field("action", &self.action)
            .finish()
    }
}

/// Scheduled jobs added to a [`ListContext`] by a list's
/// [`PostFilter`](crate::message_filters::PostFilter) stack.
#[derive(Debug)]
pub enum MailJob {
    /// Send post to recipients.
    Send {
        /// The post recipients addresses.
        recipients: Vec<Address>,
    },
    /// Send error to submitter.
    Error {
        /// Human readable description of the error.
        description: String,
    },
    /// Store post in digest for recipients.
    StoreDigest {
        /// The digest recipients addresses.
        recipients: Vec<Address>,
    },
    /// Reply with subscription confirmation to submitter.
    ConfirmSubscription {
        /// The submitter address.
        recipient: Address,
    },
    /// Reply with unsubscription confirmation to submitter.
    ConfirmUnsubscription {
        /// The submitter address.
        recipient: Address,
    },
}

/// Type of mailing list request.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ListRequest {
    /// Get help about a mailing list and its available interfaces.
    Help,
    /// Request subscription.
    Subscribe,
    /// Request removal of subscription.
    Unsubscribe,
    /// Request reception of list posts from a month-year range, inclusive.
    RetrieveArchive(String, String),
    /// Request reception of specific mailing list posts from `Message-ID`
    /// values.
    RetrieveMessages(Vec<String>),
    /// Request change in subscription settings.
    /// See [`ListSubscription`].
    ChangeSetting(String, bool),
    /// Other type of request.
    Other(String),
}

impl std::fmt::Display for ListRequest {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

impl<S: AsRef<str>> TryFrom<(S, &melib::Envelope)> for ListRequest {
    type Error = crate::Error;

    fn try_from((val, env): (S, &melib::Envelope)) -> std::result::Result<Self, Self::Error> {
        let val = val.as_ref();
        Ok(match val {
            "subscribe" => Self::Subscribe,
            "request" if env.subject().trim() == "subscribe" => Self::Subscribe,
            "unsubscribe" => Self::Unsubscribe,
            "request" if env.subject().trim() == "unsubscribe" => Self::Unsubscribe,
            "help" => Self::Help,
            "request" if env.subject().trim() == "help" => Self::Help,
            "request" => Self::Other(env.subject().trim().to_string()),
            _ => {
                // [ref:TODO] add ChangeSetting parsing
                trace!("unknown action = {} for addresses {:?}", val, env.from(),);
                Self::Other(val.trim().to_string())
            }
        })
    }
}
