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

#![allow(clippy::result_unit_err)]

//! Filters to pass each mailing list post through. Filters are functions that
//! implement the [`PostFilter`] trait that can:
//!
//! - transform post content.
//! - modify the final [`PostAction`] to take.
//! - modify the final scheduled jobs to perform. (See [`MailJob`]).
//!
//! Filters are executed in sequence like this:
//!
//! ```ignore
//! let result = filters
//!     .into_iter()
//!     .fold(Ok((&mut post, &mut list_ctx)), |p, f| {
//!         p.and_then(|(p, c)| f.feed(p, c))
//!     });
//! ```
//!
//! so the processing stops at the first returned error.

use log::trace;
use melib::Address;

use crate::{
    mail::{ListContext, MailJob, PostAction, PostEntry},
    models::{DbVal, MailingList},
    Connection,
};

impl Connection {
    /// Return the post filters of a mailing list.
    pub fn list_filters(&self, _list: &DbVal<MailingList>) -> Vec<Box<dyn PostFilter>> {
        vec![
            Box::new(FixCRLF),
            Box::new(PostRightsCheck),
            Box::new(AddListHeaders),
            Box::new(FinalizeRecipients),
        ]
    }
}

/// Filter that modifies and/or verifies a post candidate. On rejection, return
/// a string describing the error and optionally set `post.action` to `Reject`
/// or `Defer`
pub trait PostFilter {
    /// Feed post into the filter. Perform modifications to `post` and / or
    /// `ctx`, and return them with `Result::Ok` unless you want to the
    /// processing to stop and return an `Result::Err`.
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut PostEntry,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut PostEntry, &'p mut ListContext<'list>), ()>;
}

/// Check that submitter can post to list, for now it accepts everything.
pub struct PostRightsCheck;
impl PostFilter for PostRightsCheck {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut PostEntry,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut PostEntry, &'p mut ListContext<'list>), ()> {
        trace!("Running PostRightsCheck filter");
        if let Some(ref policy) = ctx.post_policy {
            if policy.announce_only {
                trace!("post policy is announce_only");
                let owner_addresses = ctx
                    .list_owners
                    .iter()
                    .map(|lo| lo.address())
                    .collect::<Vec<Address>>();
                trace!("Owner addresses are: {:#?}", &owner_addresses);
                trace!("Envelope from is: {:?}", &post.from);
                if !owner_addresses.iter().any(|addr| *addr == post.from) {
                    trace!("Envelope From does not include any owner");
                    post.action = PostAction::Reject {
                        reason: "You are not allowed to post on this list.".to_string(),
                    };
                    return Err(());
                }
            } else if policy.subscription_only {
                trace!("post policy is subscription_only");
                let email_from = post.from.get_email();
                trace!("post from is {:?}", &email_from);
                trace!("post subscriptions are {:#?}", &ctx.subscriptions);
                if !ctx.subscriptions.iter().any(|lm| lm.address == email_from) {
                    trace!("Envelope from is not subscribed to this list");
                    post.action = PostAction::Reject {
                        reason: "Only subscriptions can post to this list.".to_string(),
                    };
                    return Err(());
                }
            } else if policy.approval_needed {
                trace!("post policy says approval_needed");
                let email_from = post.from.get_email();
                trace!("post from is {:?}", &email_from);
                trace!("post subscriptions are {:#?}", &ctx.subscriptions);
                if !ctx.subscriptions.iter().any(|lm| lm.address == email_from) {
                    trace!("Envelope from is not subscribed to this list");
                    post.action = PostAction::Defer {
                        reason: "Your posting has been deferred. Approval from the list's \
                                 moderators is required before it is submitted."
                            .to_string(),
                    };
                    return Err(());
                }
            }
        }
        Ok((post, ctx))
    }
}

/// Ensure message contains only `\r\n` line terminators, required by SMTP.
pub struct FixCRLF;
impl PostFilter for FixCRLF {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut PostEntry,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut PostEntry, &'p mut ListContext<'list>), ()> {
        trace!("Running FixCRLF filter");
        use std::io::prelude::*;
        let mut new_vec = Vec::with_capacity(post.bytes.len());
        for line in post.bytes.lines() {
            new_vec.extend_from_slice(line.unwrap().as_bytes());
            new_vec.extend_from_slice(b"\r\n");
        }
        post.bytes = new_vec;
        Ok((post, ctx))
    }
}

/// Add `List-*` headers
pub struct AddListHeaders;
impl PostFilter for AddListHeaders {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut PostEntry,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut PostEntry, &'p mut ListContext<'list>), ()> {
        trace!("Running AddListHeaders filter");
        let (mut headers, body) = melib::email::parser::mail(&post.bytes).unwrap();
        let sender = format!("<{}>", ctx.list.address);
        headers.push((&b"Sender"[..], sender.as_bytes()));
        let mut subject = format!("[{}] ", ctx.list.id).into_bytes();
        if let Some((_, subj_val)) = headers
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(b"Subject"))
        {
            subject.extend(subj_val.iter().cloned());
            *subj_val = subject.as_slice();
        } else {
            headers.push((&b"Subject"[..], subject.as_slice()));
        }

        let list_id = Some(ctx.list.id_header());
        let list_help = ctx.list.help_header();
        let list_post = ctx.list.post_header(ctx.post_policy.as_deref());
        let list_unsubscribe = ctx
            .list
            .unsubscribe_header(ctx.subscription_policy.as_deref());
        let list_subscribe = ctx
            .list
            .subscribe_header(ctx.subscription_policy.as_deref());
        let list_archive = ctx.list.archive_header();

        for (hdr, val) in [
            (b"List-Id".as_slice(), &list_id),
            (b"List-Help".as_slice(), &list_help),
            (b"List-Post".as_slice(), &list_post),
            (b"List-Unsubscribe".as_slice(), &list_unsubscribe),
            (b"List-Subscribe".as_slice(), &list_subscribe),
            (b"List-Archive".as_slice(), &list_archive),
        ] {
            if let Some(val) = val {
                headers.push((hdr, val.as_bytes()));
            }
        }

        let mut new_vec = Vec::with_capacity(
            headers
                .iter()
                .map(|(h, v)| h.len() + v.len() + ": \r\n".len())
                .sum::<usize>()
                + "\r\n\r\n".len()
                + body.len(),
        );
        for (h, v) in headers {
            new_vec.extend_from_slice(h);
            new_vec.extend_from_slice(b": ");
            new_vec.extend_from_slice(v);
            new_vec.extend_from_slice(b"\r\n");
        }
        new_vec.extend_from_slice(b"\r\n\r\n");
        new_vec.extend_from_slice(body);

        post.bytes = new_vec;
        Ok((post, ctx))
    }
}

/// Adds `Archived-At` field, if configured.
pub struct ArchivedAtLink;
impl PostFilter for ArchivedAtLink {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut PostEntry,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut PostEntry, &'p mut ListContext<'list>), ()> {
        trace!("Running ArchivedAtLink filter");
        Ok((post, ctx))
    }
}

/// Assuming there are no more changes to be done on the post, it finalizes
/// which list subscriptions will receive the post in `post.action` field.
pub struct FinalizeRecipients;
impl PostFilter for FinalizeRecipients {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut PostEntry,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut PostEntry, &'p mut ListContext<'list>), ()> {
        trace!("Running FinalizeRecipients filter");
        let mut recipients = vec![];
        let mut digests = vec![];
        let email_from = post.from.get_email();
        for subscription in ctx.subscriptions {
            trace!("examining subscription {:?}", &subscription);
            if subscription.address == email_from {
                trace!("subscription is submitter");
            }
            if subscription.digest {
                if subscription.address != email_from || subscription.receive_own_posts {
                    trace!("Subscription gets digest");
                    digests.push(subscription.address());
                }
                continue;
            }
            if subscription.address != email_from || subscription.receive_own_posts {
                trace!("Subscription gets copy");
                recipients.push(subscription.address());
            }
        }
        ctx.scheduled_jobs.push(MailJob::Send { recipients });
        if !digests.is_empty() {
            ctx.scheduled_jobs.push(MailJob::StoreDigest {
                recipients: digests,
            });
        }
        post.action = PostAction::Accept;
        Ok((post, ctx))
    }
}
