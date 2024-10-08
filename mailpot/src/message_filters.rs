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

mod settings;
use log::trace;
use melib::{Address, HeaderName};
use percent_encoding::utf8_percent_encode;

use crate::{
    mail::{ListContext, MailJob, PostAction, PostEntry},
    models::{DbVal, MailingList},
    Connection, StripCarets, PATH_SEGMENT,
};

impl Connection {
    /// Return the post filters of a mailing list.
    pub fn list_filters(&self, _list: &DbVal<MailingList>) -> Vec<Box<dyn PostFilter>> {
        vec![
            Box::new(PostRightsCheck),
            Box::new(MimeReject),
            Box::new(FixCRLF),
            Box::new(AddListHeaders),
            Box::new(ArchivedAtLink),
            Box::new(AddSubjectTagPrefix),
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
        if !post.bytes.ends_with(b"\n") && new_vec.ends_with(b"\r\n") {
            new_vec.pop();
            new_vec.pop();
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

        let map_fn = |x| crate::encode_header_owned(String::into_bytes(x));

        let sender = Some(format!("<{}>", ctx.list.address)).map(map_fn);

        let list_id = Some(map_fn(ctx.list.id_header()));
        let list_help = ctx.list.help_header().map(map_fn);
        let list_post = ctx.list.post_header(ctx.post_policy.as_deref()).map(map_fn);
        let list_unsubscribe = ctx
            .list
            .unsubscribe_header(ctx.subscription_policy.as_deref())
            .map(map_fn);
        let list_subscribe = ctx
            .list
            .subscribe_header(ctx.subscription_policy.as_deref())
            .map(map_fn);
        let list_archive = ctx.list.archive_header().map(map_fn);

        for (hdr, val) in [
            (HeaderName::SENDER, &sender),
            (HeaderName::LIST_ID, &list_id),
            (HeaderName::LIST_HELP, &list_help),
            (HeaderName::LIST_POST, &list_post),
            (HeaderName::LIST_UNSUBSCRIBE, &list_unsubscribe),
            (HeaderName::LIST_SUBSCRIBE, &list_subscribe),
            (HeaderName::LIST_ARCHIVE, &list_archive),
        ] {
            if let Some(val) = val {
                headers.push((hdr, val.as_slice()));
            }
        }

        let mut new_vec = Vec::with_capacity(
            headers
                .iter()
                .map(|(h, v)| h.as_str().as_bytes().len() + v.len() + ": \r\n".len())
                .sum::<usize>()
                + "\r\n\r\n".len()
                + body.len(),
        );
        for (h, v) in headers {
            new_vec.extend_from_slice(h.as_str().as_bytes());
            new_vec.extend_from_slice(b": ");
            new_vec.extend_from_slice(v);
            new_vec.extend_from_slice(b"\r\n");
        }
        new_vec.extend_from_slice(b"\r\n");
        new_vec.extend_from_slice(body);

        post.bytes = new_vec;
        Ok((post, ctx))
    }
}

/// Add List ID prefix in Subject header (e.g. `[list-id] ...`)
pub struct AddSubjectTagPrefix;
impl PostFilter for AddSubjectTagPrefix {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut PostEntry,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut PostEntry, &'p mut ListContext<'list>), ()> {
        if let Some(mut settings) = ctx.filter_settings.remove("AddSubjectTagPrefixSettings") {
            let map = settings.as_object_mut().unwrap();
            let enabled = serde_json::from_value::<bool>(map.remove("enabled").unwrap()).unwrap();
            if !enabled {
                trace!(
                    "AddSubjectTagPrefix is disabled from settings found for list.pk = {} \
                     skipping filter",
                    ctx.list.pk
                );
                return Ok((post, ctx));
            }
        }
        trace!("Running AddSubjectTagPrefix filter");
        let (mut headers, body) = melib::email::parser::mail(&post.bytes).unwrap();
        let mut subject;
        if let Some((_, subj_val)) = headers.iter_mut().find(|(k, _)| k == HeaderName::SUBJECT) {
            subject = crate::encode_header_owned(format!("[{}] ", ctx.list.id).into_bytes());
            subject.extend(subj_val.iter().cloned());
            *subj_val = subject.as_slice();
        } else {
            subject =
                crate::encode_header_owned(format!("[{}] (no subject)", ctx.list.id).into_bytes());
            headers.push((HeaderName::SUBJECT, subject.as_slice()));
        }

        let mut new_vec = Vec::with_capacity(
            headers
                .iter()
                .map(|(h, v)| h.as_str().as_bytes().len() + v.len() + ": \r\n".len())
                .sum::<usize>()
                + "\r\n\r\n".len()
                + body.len(),
        );
        for (h, v) in headers {
            new_vec.extend_from_slice(h.as_str().as_bytes());
            new_vec.extend_from_slice(b": ");
            new_vec.extend_from_slice(v);
            new_vec.extend_from_slice(b"\r\n");
        }
        new_vec.extend_from_slice(b"\r\n");
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
        let Some(mut settings) = ctx.filter_settings.remove("ArchivedAtLinkSettings") else {
            trace!(
                "No ArchivedAtLink settings found for list.pk = {} skipping filter",
                ctx.list.pk
            );
            return Ok((post, ctx));
        };
        trace!("Running ArchivedAtLink filter");

        let map = settings.as_object_mut().unwrap();
        let template = serde_json::from_value::<String>(map.remove("template").unwrap()).unwrap();
        let preserve_carets =
            serde_json::from_value::<bool>(map.remove("preserve_carets").unwrap()).unwrap();

        let env = minijinja::Environment::new();
        let message_id = post.message_id.to_string();
        let header_val = crate::encode_header_owned(env
            .render_named_str(
                "ArchivedAtLinkSettings.template",
                &template,
                &if preserve_carets {
                    minijinja::context! {
                    msg_id =>  utf8_percent_encode(message_id.as_str(), PATH_SEGMENT).to_string()
                    }
                } else {
                    minijinja::context! {
                    msg_id =>  utf8_percent_encode(message_id.as_str().strip_carets(), PATH_SEGMENT).to_string()
                    }
                },
            )
            .map_err(|err| {
                log::error!("ArchivedAtLink: {}", err);
            })?.into_bytes());
        let (mut headers, body) = melib::email::parser::mail(&post.bytes).unwrap();
        headers.push((HeaderName::ARCHIVED_AT, header_val.as_slice()));

        let mut new_vec = Vec::with_capacity(
            headers
                .iter()
                .map(|(h, v)| h.as_str().as_bytes().len() + v.len() + ": \r\n".len())
                .sum::<usize>()
                + "\r\n\r\n".len()
                + body.len(),
        );
        for (h, v) in headers {
            new_vec.extend_from_slice(h.as_str().as_bytes());
            new_vec.extend_from_slice(b": ");
            new_vec.extend_from_slice(v);
            new_vec.extend_from_slice(b"\r\n");
        }
        new_vec.extend_from_slice(b"\r\n");
        new_vec.extend_from_slice(body);

        post.bytes = new_vec;

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

/// Allow specific MIMEs only.
pub struct MimeReject;

impl PostFilter for MimeReject {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut PostEntry,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut PostEntry, &'p mut ListContext<'list>), ()> {
        let reject = if let Some(mut settings) = ctx.filter_settings.remove("MimeRejectSettings") {
            let map = settings.as_object_mut().unwrap();
            let enabled = serde_json::from_value::<bool>(map.remove("enabled").unwrap()).unwrap();
            if !enabled {
                trace!(
                    "MimeReject is disabled from settings found for list.pk = {} skipping filter",
                    ctx.list.pk
                );
                return Ok((post, ctx));
            }
            serde_json::from_value::<Vec<String>>(map.remove("reject").unwrap())
        } else {
            return Ok((post, ctx));
        };
        trace!("Running MimeReject filter with reject = {:?}", reject);
        Ok((post, ctx))
    }
}
