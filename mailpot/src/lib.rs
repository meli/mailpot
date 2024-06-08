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

#![deny(
    missing_docs,
    rustdoc::broken_intra_doc_links,
    /* groups */
    clippy::correctness,
    clippy::suspicious,
    clippy::complexity,
    clippy::perf,
    clippy::style,
    clippy::cargo,
    clippy::nursery,
    /* restriction */
    clippy::dbg_macro,
    clippy::rc_buffer,
    clippy::as_underscore,
    clippy::assertions_on_result_states,
    /* pedantic */
    clippy::cast_lossless,
    clippy::cast_possible_wrap,
    clippy::ptr_as_ptr,
    clippy::bool_to_int_with_if,
    clippy::borrow_as_ptr,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::cast_lossless,
    clippy::cast_ptr_alignment,
    clippy::naive_bytecount
)]
#![allow(clippy::multiple_crate_versions, clippy::missing_const_for_fn)]

//! Mailing list manager library.
//!
//! Data is stored in a `sqlite3` database.
//! You can inspect the schema in [`SCHEMA`](crate::Connection::SCHEMA).
//!
//! # Usage
//!
//! `mailpot` can be used with the CLI tool in [`mailpot-cli`](mailpot-cli),
//! and/or in the web interface of the [`mailpot-web`](mailpot-web) crate.
//!
//! You can also directly use this crate as a library.
//!
//! # Example
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
//! #     administrators: vec![],
//! # };
//! #
//! # fn do_test(config: Configuration) -> mailpot::Result<()> {
//! let db = Connection::open_or_create_db(config)?.trusted();
//!
//! // Create a new mailing list
//! let list_pk = db
//!     .create_list(MailingList {
//!         pk: 0,
//!         name: "foobar chat".into(),
//!         id: "foo-chat".into(),
//!         address: "foo-chat@example.com".into(),
//!         topics: vec![],
//!         description: None,
//!         archive_url: None,
//!     })?
//!     .pk;
//!
//! db.set_list_post_policy(PostPolicy {
//!     pk: 0,
//!     list: list_pk,
//!     announce_only: false,
//!     subscription_only: true,
//!     approval_needed: false,
//!     open: false,
//!     custom: false,
//! })?;
//!
//! // Drop privileges; we can only process new e-mail and modify subscriptions from now on.
//! let mut db = db.untrusted();
//!
//! assert_eq!(db.list_subscriptions(list_pk)?.len(), 0);
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
//! assert_eq!(db.list_subscriptions(list_pk)?.len(), 1);
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
//! let envelope = melib::Envelope::from_bytes(post_bytes, None).expect("Could not parse message");
//! db.post(&envelope, post_bytes, /* dry_run */ false)?;
//!
//! assert_eq!(db.list_subscriptions(list_pk)?.len(), 1);
//! assert_eq!(db.list_posts(list_pk, None)?.len(), 1);
//! # Ok(())
//! # }
//! # do_test(config);
//! ```

/* Annotations:
 *
 * Global tags (in tagref format <https://github.com/stepchowfun/tagref>) for source code
 * annotation:
 *
 * - [tag:needs_unit_test]
 * - [tag:needs_user_doc]
 * - [tag:needs_dev_doc]
 * - [tag:FIXME]
 * - [tag:TODO]
 * - [tag:VERIFY] Verify whether this is the correct way to do something
 */

/// Error library
pub extern crate anyhow;
/// Date library
pub extern crate chrono;
/// Sql library
pub extern crate rusqlite;

/// Alias for [`chrono::DateTime<chrono::Utc>`].
pub type DateTime = chrono::DateTime<chrono::Utc>;

/// Serde
#[macro_use]
pub extern crate serde;
/// Log
pub extern crate log;
/// melib
pub extern crate melib;
/// serde_json
pub extern crate serde_json;

mod config;
mod connection;
mod errors;
pub mod mail;
pub mod message_filters;
pub mod models;
pub mod policies;
#[cfg(not(target_os = "windows"))]
pub mod postfix;
pub mod posts;
pub mod queue;
pub mod submission;
pub mod subscriptions;
mod templates;

pub use config::{Configuration, SendMail};
pub use connection::{transaction, *};
pub use errors::*;
use models::*;
pub use templates::*;

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

/// Trait for stripping carets ('<','>') from Message IDs.
pub trait StripCarets {
    /// If `self` is surrounded by carets, strip them.
    fn strip_carets(&self) -> &str;
}

impl StripCarets for &str {
    fn strip_carets(&self) -> &str {
        let mut self_ref = self.trim();
        if self_ref.starts_with('<') && self_ref.ends_with('>') {
            self_ref = &self_ref[1..self_ref.len().saturating_sub(1)];
        }
        self_ref
    }
}

/// Trait for stripping carets ('<','>') from Message IDs inplace.
pub trait StripCaretsInplace {
    /// If `self` is surrounded by carets, strip them.
    fn strip_carets_inplace(self) -> Self;
}

impl StripCaretsInplace for &str {
    fn strip_carets_inplace(self) -> Self {
        let mut self_ref = self.trim();
        if self_ref.starts_with('<') && self_ref.ends_with('>') {
            self_ref = &self_ref[1..self_ref.len().saturating_sub(1)];
        }
        self_ref
    }
}

impl StripCaretsInplace for String {
    fn strip_carets_inplace(mut self) -> Self {
        if self.starts_with('<') && self.ends_with('>') {
            self.drain(0..1);
            let len = self.len();
            self.drain(len.saturating_sub(1)..len);
        }
        self
    }
}

use percent_encoding::CONTROLS;
pub use percent_encoding::{utf8_percent_encode, AsciiSet};

// from https://github.com/servo/rust-url/blob/master/url/src/parser.rs
const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
const PATH: &AsciiSet = &FRAGMENT.add(b'#').add(b'?').add(b'{').add(b'}');

/// Set for percent encoding URL components.
pub const PATH_SEGMENT: &AsciiSet = &PATH.add(b'/').add(b'%');

mod helpers {
    use std::borrow::Cow;

    use data_encoding::Encoding;

    fn base64_encoding() -> Encoding {
        let mut spec = data_encoding::BASE64_MIME.specification();
        spec.ignore.clear();
        spec.wrap.width = 0;
        spec.wrap.separator.clear();
        spec.encoding().unwrap()
    }

    /// Ensure `value` is in appropriate representation to be a header value.
    pub fn encode_header(value: &'_ [u8]) -> Cow<'_, [u8]> {
        if value.iter().all(|&b| b.is_ascii_graphic() || b == b' ') {
            return Cow::Borrowed(value);
        }
        Cow::Owned(_encode_header(value))
    }

    /// Same as [`encode_header`] but for owned bytes.
    pub fn encode_header_owned(value: Vec<u8>) -> Vec<u8> {
        if value.iter().all(|&b| b.is_ascii_graphic() || b == b' ') {
            return value;
        }
        _encode_header(&value)
    }

    fn _encode_header(value: &[u8]) -> Vec<u8> {
        let mut ret = Vec::with_capacity(value.len());
        let base64_mime = base64_encoding();
        let mut is_current_window_ascii = true;
        let mut current_window_start = 0;
        {
            for (idx, g) in value.iter().copied().enumerate() {
                match (g.is_ascii(), is_current_window_ascii) {
                    (true, true) => {
                        if g.is_ascii_graphic() || g == b' ' {
                            ret.push(g);
                        } else {
                            current_window_start = idx;
                            is_current_window_ascii = false;
                        }
                    }
                    (true, false) => {
                        /* If !g.is_whitespace()
                         *
                         * Whitespaces inside encoded tokens must be greedily taken,
                         * instead of splitting each non-ascii word into separate encoded tokens. */
                        if g != b' ' && !g.is_ascii_control() {
                            ret.extend_from_slice(
                                format!(
                                    "=?UTF-8?B?{}?=",
                                    base64_mime.encode(&value[current_window_start..idx]).trim()
                                )
                                .as_bytes(),
                            );
                            if idx != value.len() - 1
                                && ((idx == 0)
                                    ^ (!value[idx - 1].is_ascii_control()
                                        && !value[idx - 1] != b' '))
                            {
                                ret.push(b' ');
                            }
                            is_current_window_ascii = true;
                            current_window_start = idx;
                            ret.push(g);
                        }
                    }
                    (false, true) => {
                        current_window_start = idx;
                        is_current_window_ascii = false;
                    }
                    /* RFC2047 recommends:
                     * 'While there is no limit to the length of a multiple-line header field,
                     * each line of a header field that contains one or more
                     * 'encoded-word's is limited to 76 characters.'
                     * This is a rough compliance.
                     */
                    (false, false) if (((4 * (idx - current_window_start) / 3) + 3) & !3) > 33 => {
                        ret.extend_from_slice(
                            format!(
                                "=?UTF-8?B?{}?=",
                                base64_mime.encode(&value[current_window_start..idx]).trim()
                            )
                            .as_bytes(),
                        );
                        if idx != value.len() - 1 {
                            ret.push(b' ');
                        }
                        current_window_start = idx;
                    }
                    (false, false) => {}
                }
            }
        }
        /* If the last part of the header value is encoded, it won't be pushed inside
         * the previous for block */
        if !is_current_window_ascii {
            ret.extend_from_slice(
                format!(
                    "=?UTF-8?B?{}?=",
                    base64_mime.encode(&value[current_window_start..]).trim()
                )
                .as_bytes(),
            );
        }
        ret
    }
}

pub use helpers::*;
