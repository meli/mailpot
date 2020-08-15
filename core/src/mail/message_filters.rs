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

///Filter that modifies and/or verifies a post candidate. On rejection, return a string
///describing the error and optionally set `post.action` to `Reject` or `Defer`
pub trait PostFilter {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut Post,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut Post, &'p mut ListContext<'list>), ()>;
}

///Check that submitter can post to list, for now it accepts everything.
pub struct PostRightsCheck;
impl PostFilter for PostRightsCheck {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut Post,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut Post, &'p mut ListContext<'list>), ()> {
        trace!("Running PostRightsCheck filter");
        if let Some(ref policy) = ctx.policy {
            if policy.announce_only {
                trace!("post policy is announce_only");
                let owner_addresses = ctx
                    .list_owners
                    .iter()
                    .map(|lo| lo.into_address())
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
            } else if policy.subscriber_only {
                trace!("post policy is subscriber_only");
                let email_from = post.from.get_email();
                trace!("post from is {:?}", &email_from);
                trace!("post memberships are {:#?}", &ctx.memberships);
                if !ctx.memberships.iter().any(|lm| lm.address == email_from) {
                    trace!("Envelope from is not subscribed to this list");
                    post.action = PostAction::Reject {
                        reason: "Only subscribers can post to this list.".to_string(),
                    };
                    return Err(());
                }
            } else if policy.approval_needed {
                trace!("post policy says approval_needed");
                post.action = PostAction::Defer {
                    reason: "Your posting has been deferred. Approval from the list's moderators is required before it is submitted.".to_string(),
                };
            }
        }
        Ok((post, ctx))
    }
}

///Ensure message contains only `\r\n` line terminators, required by SMTP.
pub struct FixCRLF;
impl PostFilter for FixCRLF {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut Post,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut Post, &'p mut ListContext<'list>), ()> {
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

///Add `List-*` headers
pub struct AddListHeaders;
impl PostFilter for AddListHeaders {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut Post,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut Post, &'p mut ListContext<'list>), ()> {
        trace!("Running AddListHeaders filter");
        let (mut headers, body) = melib::email::parser::mail(&post.bytes).unwrap();
        let list_id = ctx.list.list_id();
        headers.push((&b"List-ID"[..], list_id.as_bytes()));
        let list_post = ctx.list.list_post();
        let list_unsubscribe = ctx.list.list_unsubscribe();
        let list_archive = ctx.list.list_archive();
        if let Some(post) = list_post.as_ref() {
            headers.push((&b"List-Post"[..], post.as_bytes()));
        }
        if let Some(unsubscribe) = list_unsubscribe.as_ref() {
            headers.push((&b"List-Unsubscribe"[..], unsubscribe.as_bytes()));
        }
        if let Some(archive) = list_archive.as_ref() {
            headers.push((&b"List-Archive"[..], archive.as_bytes()));
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

///Adds `Archived-At` field, if configured.
pub struct ArchivedAtLink;
impl PostFilter for ArchivedAtLink {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut Post,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut Post, &'p mut ListContext<'list>), ()> {
        trace!("Running ArchivedAtLink filter");
        Ok((post, ctx))
    }
}

///Assuming there are no more changes to be done on the post, it finalizes which list members
///will receive the post in `post.action` field.
pub struct FinalizeRecipients;
impl PostFilter for FinalizeRecipients {
    fn feed<'p, 'list>(
        self: Box<Self>,
        post: &'p mut Post,
        ctx: &'p mut ListContext<'list>,
    ) -> std::result::Result<(&'p mut Post, &'p mut ListContext<'list>), ()> {
        trace!("Running FinalizeRecipients filter");
        let mut recipients = vec![];
        let mut digests = vec![];
        let email_from = post.from.get_email();
        for member in ctx.memberships {
            trace!("examining member {:?}", &member);
            if member.address != email_from {
                trace!("member is submitter");
            }
            if member.digest {
                if member.address != email_from || member.receive_own_posts {
                    trace!("Member gets digest");
                    digests.push(member.into_address());
                }
                continue;
            }
            if member.address != email_from || member.receive_own_posts {
                trace!("Member gets copy");
                recipients.push(member.into_address());
            }
            // TODO:
            // - check for duplicates (To,Cc,Bcc)
            // - send confirmation to submitter
        }
        ctx.scheduled_jobs.push(MailJob::Send {
            message_pk: post.pk,
            recipients,
        });
        if !digests.is_empty() {
            ctx.scheduled_jobs.push(MailJob::StoreDigest {
                message_pk: post.pk,
                recipients: digests,
            });
        }
        post.action = PostAction::Accept;
        Ok((post, ctx))
    }
}
