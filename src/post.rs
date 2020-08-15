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

#[derive(Debug)]
pub enum PostAction {
    Accept {
        recipients: Vec<Address>,
        digests: Vec<Address>,
    },
    Reject {
        reason: String,
    },
    Defer {
        reason: String,
    },
}

///Post to be considered by the list's `PostFilter` stack.
pub struct Post<'list> {
    pub list: &'list mut MailingList,
    pub from: Address,
    pub memberships: &'list [ListMembership],
    pub bytes: Vec<u8>,
    pub to: Vec<Address>,
    pub action: PostAction,
}

impl<'list> core::fmt::Debug for Post<'list> {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        fmt.debug_struct("Post")
            .field("list", &self.list)
            .field("from", &self.from)
            .field("members", &format_args!("{}", self.memberships.len()))
            .field("bytes", &format_args!("{}", self.bytes.len()))
            .field("to", &self.to.as_slice())
            .field("action", &self.action)
            .finish()
    }
}

///Filter that modifies and/or verifies a post candidate. On rejection, return a string
///describing the error and optionally set `post.action` to `Reject` or `Defer`
pub trait PostFilter {
    fn feed<'list>(
        self: Box<Self>,
        post: &'list mut Post<'list>,
    ) -> std::result::Result<&'list mut Post<'list>, String>;
}

///Check that submitter can post to list, for now it accepts everything.
pub struct PostRightsCheck;
impl PostFilter for PostRightsCheck {
    fn feed<'list>(
        self: Box<Self>,
        post: &'list mut Post<'list>,
    ) -> std::result::Result<&'list mut Post<'list>, String> {
        Ok(post)
    }
}

///Ensure message contains only `\r\n` line terminators, required by SMTP.
pub struct FixCRLF;
impl PostFilter for FixCRLF {
    fn feed<'list>(
        self: Box<Self>,
        post: &'list mut Post<'list>,
    ) -> std::result::Result<&'list mut Post<'list>, String> {
        use std::io::prelude::*;
        let mut new_vec = Vec::with_capacity(post.bytes.len());
        for line in post.bytes.lines() {
            new_vec.extend_from_slice(line.unwrap().as_bytes());
            new_vec.extend_from_slice(b"\r\n");
        }
        post.bytes = new_vec;
        Ok(post)
    }
}

///Add `List-*` headers
pub struct AddListHeaders;
impl PostFilter for AddListHeaders {
    fn feed<'list>(
        self: Box<Self>,
        post: &'list mut Post<'list>,
    ) -> std::result::Result<&'list mut Post<'list>, String> {
        let (mut headers, body) = melib::email::parser::mail(&post.bytes).unwrap();
        let list_id = post.list.list_id();
        headers.push((&b"List-ID"[..], list_id.as_bytes()));
        let list_post = post.list.list_post();
        let list_unsubscribe = post.list.list_unsubscribe();
        let list_archive = post.list.list_archive();
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
            new_vec.extend_from_slice(&h);
            new_vec.extend_from_slice(b": ");
            new_vec.extend_from_slice(&v);
            new_vec.extend_from_slice(b"\r\n");
        }
        new_vec.extend_from_slice(b"\r\n\r\n");
        new_vec.extend_from_slice(&body);

        post.bytes = new_vec;
        Ok(post)
    }
}

///Adds `Archived-At` field, if configured.
pub struct ArchivedAtLink;
impl PostFilter for ArchivedAtLink {
    fn feed<'list>(
        self: Box<Self>,
        post: &'list mut Post<'list>,
    ) -> std::result::Result<&'list mut Post<'list>, String> {
        Ok(post)
    }
}

///Assuming there are no more changes to be done on the post, it finalizes which list members
///will receive the post in `post.action` field.
pub struct FinalizeRecipients;
impl PostFilter for FinalizeRecipients {
    fn feed<'list>(
        self: Box<Self>,
        post: &'list mut Post<'list>,
    ) -> std::result::Result<&'list mut Post<'list>, String> {
        let mut recipients = vec![];
        let mut digests = vec![];
        let email_from = post.from.get_email();
        for member in post.memberships {
            if member.digest {
                if (member.address == email_from && member.receive_own_posts)
                    || (member.address != email_from)
                {
                    digests.push(member.into_address());
                }
                continue;
            }
            if (member.address == email_from && member.receive_own_posts)
                || (member.address != email_from)
            {
                recipients.push(member.into_address());
            }
            // TODO:
            // - check for duplicates (To,Cc,Bcc)
            // - send confirmation to submitter
        }
        post.action = PostAction::Accept {
            recipients,
            digests,
        };
        Ok(post)
    }
}
