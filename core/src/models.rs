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

//! Database models: [`MailingList`], [`ListOwner`], [`ListSubscription`],
//! [`PostPolicy`], [`SubscriptionPolicy`] and [`Post`].

use super::*;
pub mod changesets;

use std::borrow::Cow;

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

    /// Unwrap inner value.
    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::ops::Deref for DbVal<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> std::ops::DerefMut for DbVal<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
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
    /// Mailing list ID (what appears in the subject tag, e.g. `[mailing-list]
    /// New post!`).
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

    #[inline]
    /// Request subaddress.
    pub fn request_subaddr(&self) -> String {
        let p = self.address.split('@').collect::<Vec<&str>>();
        format!("{}+request@{}", p[0], p[1])
    }

    /// Value of `List-Id` header.
    ///
    /// See RFC2919 Section 3: <https://www.rfc-editor.org/rfc/rfc2919>
    pub fn id_header(&self) -> String {
        let p = self.address.split('@').collect::<Vec<&str>>();
        format!(
            "{}{}<{}.{}>",
            self.description.as_deref().unwrap_or(""),
            self.description.as_ref().map(|_| " ").unwrap_or(""),
            self.id,
            p[1]
        )
    }

    /// Value of `List-Help` header.
    ///
    /// See RFC2369 Section 3.1: <https://www.rfc-editor.org/rfc/rfc2369#section-3.1>
    pub fn help_header(&self) -> Option<String> {
        Some(format!("<mailto:{}?subject=help>", self.request_subaddr()))
    }

    /// Value of `List-Post` header.
    ///
    /// See RFC2369 Section 3.4: <https://www.rfc-editor.org/rfc/rfc2369#section-3.4>
    pub fn post_header(&self, policy: Option<&PostPolicy>) -> Option<String> {
        Some(policy.map_or_else(
            || "NO".to_string(),
            |p| {
                if p.announce_only {
                    "NO".to_string()
                } else {
                    format!("<mailto:{}>", self.address)
                }
            },
        ))
    }

    /// Value of `List-Unsubscribe` header.
    ///
    /// See RFC2369 Section 3.2: <https://www.rfc-editor.org/rfc/rfc2369#section-3.2>
    pub fn unsubscribe_header(&self, policy: Option<&SubscriptionPolicy>) -> Option<String> {
        policy.map_or_else(
            || None,
            |p| {
                if p.open {
                    None
                } else {
                    Some(format!(
                        "<mailto:{}?subject=unsubscribe>",
                        self.request_subaddr()
                    ))
                }
            },
        )
    }

    /// Value of `List-Subscribe` header.
    ///
    /// See RFC2369 Section 3.3: <https://www.rfc-editor.org/rfc/rfc2369#section-3.3>
    pub fn subscribe_header(&self, policy: Option<&SubscriptionPolicy>) -> Option<String> {
        policy.map_or_else(
            || None,
            |p| {
                if p.open {
                    None
                } else {
                    Some(format!(
                        "<mailto:{}?subject=subscribe>",
                        self.request_subaddr()
                    ))
                }
            },
        )
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
    pub fn unsubscription_mailto(&self) -> MailtoAddress {
        MailtoAddress {
            address: self.request_subaddr(),
            subject: Some("unsubscribe".to_string()),
        }
    }

    /// List subscribe action as a [`MailtoAddress`](super::MailtoAddress).
    pub fn subscription_mailto(&self) -> MailtoAddress {
        MailtoAddress {
            address: self.request_subaddr(),
            subject: Some("subscribe".to_string()),
        }
    }

    /// List owner as a [`MailtoAddress`](super::MailtoAddress).
    pub fn owner_mailto(&self) -> MailtoAddress {
        let p = self.address.split('@').collect::<Vec<&str>>();
        MailtoAddress {
            address: format!("{}+owner@{}", p[0], p[1]),
            subject: None,
        }
    }

    /// List archive url value.
    pub fn archive_url(&self) -> Option<&str> {
        self.archive_url.as_deref()
    }

    /// Insert all available list headers.
    pub fn insert_headers(
        &self,
        draft: &mut melib::Draft,
        post_policy: Option<&PostPolicy>,
        subscription_policy: Option<&SubscriptionPolicy>,
    ) {
        for (hdr, val) in [
            ("List-Id", Some(self.id_header())),
            ("List-Help", self.help_header()),
            ("List-Post", self.post_header(post_policy)),
            (
                "List-Unsubscribe",
                self.unsubscribe_header(subscription_policy),
            ),
            ("List-Subscribe", self.subscribe_header(subscription_policy)),
            ("List-Archive", self.archive_header()),
        ] {
            if let Some(val) = val {
                draft
                    .headers
                    .insert(melib::HeaderName::new_unchecked(hdr), val);
            }
        }
    }

    /// Generate help e-mail body containing information on how to subscribe,
    /// unsubscribe, post and how to contact the list owners.
    pub fn generate_help_email(
        &self,
        post_policy: Option<&PostPolicy>,
        subscription_policy: Option<&SubscriptionPolicy>,
    ) -> String {
        format!(
            "Help for {list_name}\n\n{subscribe}\n\n{post}\n\nTo contact the list owners, send an \
             e-mail to {contact}\n",
            list_name = self.name,
            subscribe = subscription_policy.map_or(
                Cow::Borrowed("This list is not open to subscriptions."),
                |p| if p.open {
                    Cow::Owned(format!(
                        "Anyone can subscribe without restrictions. Send an e-mail to {} with the \
                         subject `subscribe`.",
                        self.request_subaddr(),
                    ))
                } else if p.manual {
                    Cow::Borrowed(
                        "The list owners must manually add you to the list of subscriptions.",
                    )
                } else if p.request {
                    Cow::Owned(format!(
                        "Anyone can request to subscribe. Send an e-mail to {} with the subject \
                         `subscribe` and a confirmation will be sent to you when your request is \
                         approved.",
                        self.request_subaddr(),
                    ))
                } else {
                    Cow::Borrowed("Please contact the list owners for details on how to subscribe.")
                }
            ),
            post = post_policy.map_or(Cow::Borrowed("This list does not allow posting."), |p| {
                if p.announce_only {
                    Cow::Borrowed(
                        "This list is announce only, which means that you can only receive posts \
                         from the list owners.",
                    )
                } else if p.subscription_only {
                    Cow::Owned(format!(
                        "Only list subscriptions can post to this list. Send your post to {}",
                        self.address
                    ))
                } else if p.approval_needed {
                    Cow::Owned(format!(
                        "Anyone can post, but approval from list owners is required if they are \
                         not subscribed. Send your post to {}",
                        self.address
                    ))
                } else {
                    Cow::Borrowed("This list does not allow posting.")
                }
            }),
            contact = self.owner_mailto().address,
        )
    }
}

/// A mailing list subscription entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListSubscription {
    /// Database primary key.
    pub pk: i64,
    /// Mailing list foreign key (See [`MailingList`]).
    pub list: i64,
    /// Subscription's e-mail address.
    pub address: String,
    /// Subscription's name, optional.
    pub name: Option<String>,
    /// Subscription's account foreign key, optional.
    pub account: Option<i64>,
    /// Whether this subscription is enabled.
    pub enabled: bool,
    /// Whether the e-mail address is verified.
    pub verified: bool,
    /// Whether subscription wishes to receive list posts as a periodical digest
    /// e-mail.
    pub digest: bool,
    /// Whether subscription wishes their e-mail address hidden from public
    /// view.
    pub hide_address: bool,
    /// Whether subscription wishes to receive mailing list post duplicates,
    /// i.e. posts addressed to them and the mailing list to which they are
    /// subscribed.
    pub receive_duplicates: bool,
    /// Whether subscription wishes to receive their own mailing list posts from
    /// the mailing list, as a confirmation.
    pub receive_own_posts: bool,
    /// Whether subscription wishes to receive a plain confirmation for their
    /// own mailing list posts.
    pub receive_confirmation: bool,
}

impl std::fmt::Display for ListSubscription {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            fmt,
            "{} [digest: {}, hide_address: {} verified: {} {}]",
            self.address(),
            self.digest,
            self.hide_address,
            self.verified,
            if self.enabled {
                "enabled"
            } else {
                "not enabled"
            },
        )
    }
}

impl ListSubscription {
    /// Subscription address as a [`melib::Address`]
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
    /// Whether the policy is announce only (Only list owners can submit posts,
    /// and everyone will receive them).
    pub announce_only: bool,
    /// Whether the policy is "subscription only" (Only list subscriptions can
    /// post).
    pub subscription_only: bool,
    /// Whether the policy is "approval needed" (Anyone can post, but approval
    /// from list owners is required if they are not subscribed).
    pub approval_needed: bool,
    /// Whether the policy is "open" (Anyone can post, but approval from list
    /// owners is required. Subscriptions are not enabled).
    pub open: bool,
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

impl From<ListOwner> for ListSubscription {
    fn from(val: ListOwner) -> Self {
        Self {
            pk: 0,
            list: val.list,
            address: val.address,
            name: val.name,
            account: None,
            digest: false,
            hide_address: false,
            receive_duplicates: true,
            receive_own_posts: false,
            receive_confirmation: true,
            enabled: true,
            verified: true,
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
    /// Envelope `From` of post.
    pub envelope_from: Option<String>,
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

/// A mailing list subscription policy entry.
///
/// Only one of the policy boolean flags must be set to true.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubscriptionPolicy {
    /// Database primary key.
    pub pk: i64,
    /// Mailing list foreign key (See [`MailingList`]).
    pub list: i64,
    /// Send confirmation e-mail when subscription is finalized.
    pub send_confirmation: bool,
    /// Anyone can subscribe without restrictions.
    pub open: bool,
    /// Only list owners can manually add subscriptions.
    pub manual: bool,
    /// Anyone can request to subscribe.
    pub request: bool,
    /// Allow subscriptions, but handle it manually.
    pub custom: bool,
}

impl std::fmt::Display for SubscriptionPolicy {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/// An account entry.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Account {
    /// Database primary key.
    pub pk: i64,
    /// Accounts's display name, optional.
    pub name: Option<String>,
    /// Account's e-mail address.
    pub address: String,
    /// GPG public key.
    pub public_key: Option<String>,
    /// SSH public key.
    pub password: String,
    /// Whether this account is enabled.
    pub enabled: bool,
}

impl std::fmt::Display for Account {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/// A mailing list subscription candidate.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListCandidateSubscription {
    /// Database primary key.
    pub pk: i64,
    /// Mailing list foreign key (See [`MailingList`]).
    pub list: i64,
    /// Subscription's e-mail address.
    pub address: String,
    /// Subscription's name, optional.
    pub name: Option<String>,
    /// Accepted, foreign key on [`ListSubscription`].
    pub accepted: Option<i64>,
}

impl std::fmt::Display for ListCandidateSubscription {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            fmt,
            "List_pk: {} name: {:?} address: {} accepted: {:?}",
            self.list, self.name, self.address, self.accepted,
        )
    }
}
