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

//! Submit e-mail through SMTP.

use std::{future::Future, pin::Pin};

use melib::smtp::*;

use crate::{errors::*, Connection, QueueEntry};

type ResultFuture<T> = Result<Pin<Box<dyn Future<Output = Result<T>> + Send + 'static>>>;

impl Connection {
    /// Return an SMTP connection handle if the database connection has one
    /// configured.
    pub fn new_smtp_connection(&self) -> ResultFuture<SmtpConnection> {
        if let crate::SendMail::Smtp(ref smtp_conf) = &self.conf().send_mail {
            let smtp_conf = smtp_conf.clone();
            Ok(Box::pin(async move {
                Ok(SmtpConnection::new_connection(smtp_conf).await?)
            }))
        } else {
            Err("No SMTP configuration found: use the shell command instead.".into())
        }
    }

    /// Submit queue items from `values` to their recipients.
    pub async fn submit(
        smtp_connection: &mut melib::smtp::SmtpConnection,
        message: &QueueEntry,
        dry_run: bool,
    ) -> Result<()> {
        let QueueEntry {
            ref comment,
            ref to_addresses,
            ref from_address,
            ref subject,
            ref message,
            ..
        } = message;
        log::info!(
            "Sending message from {from_address} to {to_addresses} with subject {subject:?} and \
             comment {comment:?}",
        );
        let recipients = melib::Address::list_try_from(to_addresses)
            .context(format!("Could not parse {to_addresses:?}"))?;
        if dry_run {
            log::warn!("Dry run is true, not actually submitting anything to SMTP server.");
        } else {
            smtp_connection
                .mail_transaction(&String::from_utf8_lossy(message), Some(&recipients))
                .await?;
        }
        Ok(())
    }
}
