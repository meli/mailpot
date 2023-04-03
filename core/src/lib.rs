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
#![warn(missing_docs)]
//! Mailing list manager library.
//!
//! ```
//! use mailpot::{models::*, Configuration, Connection, SendMail};
//! # use tempfile::TempDir;
//!
//! # let tmp_dir = TempDir::new().unwrap();
//! # let db_path = tmp_dir.path().join("mpot.db");
//! # let config = Configuration {
//! #     send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
//! #     db_path: db_path.clone(),
//! #     data_path: tmp_dir.path().to_path_buf(),
//! # };
//! #
//! # fn do_test(config: Configuration) -> mailpot::Result<()> {
//! let db = Connection::open_or_create_db(config)?.trusted();
//!
//! // Create a new mailing list
//! let list_pk = db.create_list(MailingList {
//!     pk: 0,
//!     name: "foobar chat".into(),
//!     id: "foo-chat".into(),
//!     address: "foo-chat@example.com".into(),
//!     description: None,
//!     archive_url: None,
//! })?.pk;
//!
//! db.set_list_policy(
//!     PostPolicy {
//!         pk: 0,
//!         list: list_pk,
//!         announce_only: false,
//!         subscriber_only: true,
//!         approval_needed: false,
//!         no_subscriptions: false,
//!         custom: false,
//!     },
//! )?;
//!
//! // Drop privileges; we can only process new e-mail and modify memberships from now on.
//! let db = db.untrusted();
//!
//! assert_eq!(db.list_members(list_pk)?.len(), 0);
//! assert_eq!(db.list_posts(list_pk, None)?.len(), 0);
//!
//! // Process a subscription request e-mail
//! let subscribe_bytes = b"From: Name <user@example.com>
//! To: <foo-chat+subscribe@example.com>
//! Subject: subscribe
//! Date: Thu, 29 Oct 2020 13:58:16 +0000
//! Message-ID: <1@example.com>
//!
//! ";
//! let envelope = melib::Envelope::from_bytes(subscribe_bytes, None)?;
//! db.post(&envelope, subscribe_bytes, /* dry_run */ false)?;
//!
//! assert_eq!(db.list_members(list_pk)?.len(), 1);
//! assert_eq!(db.list_posts(list_pk, None)?.len(), 0);
//!
//! // Process a post
//! let post_bytes = b"From: Name <user@example.com>
//! To: <foo-chat@example.com>
//! Subject: my first post
//! Date: Thu, 29 Oct 2020 14:01:09 +0000
//! Message-ID: <2@example.com>
//!
//! Hello
//! ";
//! let envelope =
//!     melib::Envelope::from_bytes(post_bytes, None).expect("Could not parse message");
//! db.post(&envelope, post_bytes, /* dry_run */ false)?;
//!
//! assert_eq!(db.list_members(list_pk)?.len(), 1);
//! assert_eq!(db.list_posts(list_pk, None)?.len(), 1);
//! # Ok(())
//! # }
//! # do_test(config);
//! ```

#[macro_use]
extern crate error_chain;
extern crate anyhow;

#[macro_use]
pub extern crate serde;
pub extern crate log;
pub extern crate melib;
pub extern crate serde_json;

use log::{info, trace};

mod config;
pub mod mail;
pub mod models;
use models::*;
mod db;
mod errors;

pub use config::{Configuration, SendMail};
pub use db::*;
pub use errors::*;

/// A `mailto:` value.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MailtoAddress {
    /// E-mail address.
    pub address: String,
    /// Optional subject value.
    pub subject: Option<String>,
}

#[doc = include_str!("../../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
