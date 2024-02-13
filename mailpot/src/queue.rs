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

//! # Queues

use std::borrow::Cow;

use melib::Envelope;

use crate::{errors::*, models::DbVal, Connection, DateTime};

/// In-database queues of mail.
#[derive(Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Queue {
    /// Messages that have been received but not yet processed, await
    /// processing in the `maildrop` queue. Messages can be added to the
    /// `maildrop` queue even when mailpot is not running.
    Maildrop,
    /// List administrators may introduce rules for emails to be placed
    /// indefinitely in the `hold` queue. Messages placed in the `hold`
    /// queue stay there until the administrator intervenes. No periodic
    /// delivery attempts are made for messages in the `hold` queue.
    Hold,
    /// When all the deliverable recipients for a message are delivered, and for
    /// some recipients delivery failed for a transient reason (it might
    /// succeed later), the message is placed in the `deferred` queue.
    Deferred,
    /// Invalid received or generated e-mail saved for debug and troubleshooting
    /// reasons.
    Corrupt,
    /// Emails that must be sent as soon as possible.
    Out,
    /// Error queue
    Error,
}

impl std::str::FromStr for Queue {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.trim() {
            s if s.eq_ignore_ascii_case(stringify!(Maildrop)) => Self::Maildrop,
            s if s.eq_ignore_ascii_case(stringify!(Hold)) => Self::Hold,
            s if s.eq_ignore_ascii_case(stringify!(Deferred)) => Self::Deferred,
            s if s.eq_ignore_ascii_case(stringify!(Corrupt)) => Self::Corrupt,
            s if s.eq_ignore_ascii_case(stringify!(Out)) => Self::Out,
            s if s.eq_ignore_ascii_case(stringify!(Error)) => Self::Error,
            other => return Err(Error::new_external(format!("Invalid Queue name: {other}."))),
        })
    }
}

impl Queue {
    /// Returns the name of the queue used in the database schema.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Maildrop => "maildrop",
            Self::Hold => "hold",
            Self::Deferred => "deferred",
            Self::Corrupt => "corrupt",
            Self::Out => "out",
            Self::Error => "error",
        }
    }

    /// Returns all possible variants as `&'static str`
    pub const fn possible_values() -> &'static [&'static str] {
        const VALUES: &[&str] = &[
            Queue::Maildrop.as_str(),
            Queue::Hold.as_str(),
            Queue::Deferred.as_str(),
            Queue::Corrupt.as_str(),
            Queue::Out.as_str(),
            Queue::Error.as_str(),
        ];
        VALUES
    }
}

impl std::fmt::Display for Queue {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{}", self.as_str())
    }
}

/// A queue entry.
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct QueueEntry {
    /// Database primary key.
    pub pk: i64,
    /// Owner queue.
    pub queue: Queue,
    /// Related list foreign key, optional.
    pub list: Option<i64>,
    /// Entry comment, optional.
    pub comment: Option<String>,
    /// Entry recipients in rfc5322 format.
    pub to_addresses: String,
    /// Entry submitter in rfc5322 format.
    pub from_address: String,
    /// Entry subject.
    pub subject: String,
    /// Entry Message-ID in rfc5322 format.
    pub message_id: String,
    /// Message in rfc5322 format as bytes.
    pub message: Vec<u8>,
    /// Unix timestamp of date.
    pub timestamp: u64,
    /// Datetime as string.
    pub datetime: DateTime,
}

impl std::fmt::Display for QueueEntry {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

impl std::fmt::Debug for QueueEntry {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct(stringify!(QueueEntry))
            .field("pk", &self.pk)
            .field("queue", &self.queue)
            .field("list", &self.list)
            .field("comment", &self.comment)
            .field("to_addresses", &self.to_addresses)
            .field("from_address", &self.from_address)
            .field("subject", &self.subject)
            .field("message_id", &self.message_id)
            .field("message length", &self.message.len())
            .field(
                "message",
                &format!("{:.15}", String::from_utf8_lossy(&self.message)),
            )
            .field("timestamp", &self.timestamp)
            .field("datetime", &self.datetime)
            .finish()
    }
}

impl QueueEntry {
    /// Create new entry.
    pub fn new(
        queue: Queue,
        list: Option<i64>,
        env: Option<Cow<'_, Envelope>>,
        raw: &[u8],
        comment: Option<String>,
    ) -> Result<Self> {
        let env = env
            .map(Ok)
            .unwrap_or_else(|| melib::Envelope::from_bytes(raw, None).map(Cow::Owned))?;
        let now = chrono::offset::Utc::now();
        Ok(Self {
            pk: -1,
            list,
            queue,
            comment,
            to_addresses: env.field_to_to_string(),
            from_address: env.field_from_to_string(),
            subject: env.subject().to_string(),
            message_id: env.message_id().to_string(),
            message: raw.to_vec(),
            timestamp: now.timestamp() as u64,
            datetime: now,
        })
    }
}

impl Connection {
    /// Insert a received email into a queue.
    pub fn insert_to_queue(&self, mut entry: QueueEntry) -> Result<DbVal<QueueEntry>> {
        log::trace!("Inserting to queue: {entry}");
        let mut stmt = self.connection.prepare(
            "INSERT INTO queue(which, list, comment, to_addresses, from_address, subject, \
             message_id, message, timestamp, datetime) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             RETURNING pk;",
        )?;
        let pk = stmt.query_row(
            rusqlite::params![
                entry.queue.as_str(),
                &entry.list,
                &entry.comment,
                &entry.to_addresses,
                &entry.from_address,
                &entry.subject,
                &entry.message_id,
                &entry.message,
                &entry.timestamp,
                &entry.datetime,
            ],
            |row| {
                let pk: i64 = row.get("pk")?;
                Ok(pk)
            },
        )?;
        entry.pk = pk;
        Ok(DbVal(entry, pk))
    }

    /// Fetch all queue entries.
    pub fn queue(&self, queue: Queue) -> Result<Vec<DbVal<QueueEntry>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM queue WHERE which = ?;")?;
        let iter = stmt.query_map([&queue.as_str()], |row| {
            let pk = row.get::<_, i64>("pk")?;
            Ok(DbVal(
                QueueEntry {
                    pk,
                    queue,
                    list: row.get::<_, Option<i64>>("list")?,
                    comment: row.get::<_, Option<String>>("comment")?,
                    to_addresses: row.get::<_, String>("to_addresses")?,
                    from_address: row.get::<_, String>("from_address")?,
                    subject: row.get::<_, String>("subject")?,
                    message_id: row.get::<_, String>("message_id")?,
                    message: row.get::<_, Vec<u8>>("message")?,
                    timestamp: row.get::<_, u64>("timestamp")?,
                    datetime: row.get::<_, DateTime>("datetime")?,
                },
                pk,
            ))
        })?;

        let mut ret = vec![];
        for item in iter {
            let item = item?;
            ret.push(item);
        }
        Ok(ret)
    }

    /// Delete queue entries returning the deleted values.
    pub fn delete_from_queue(&self, queue: Queue, index: Vec<i64>) -> Result<Vec<QueueEntry>> {
        let tx = self.savepoint(Some(stringify!(delete_from_queue)))?;

        let cl = |row: &rusqlite::Row<'_>| {
            Ok(QueueEntry {
                pk: -1,
                queue,
                list: row.get::<_, Option<i64>>("list")?,
                comment: row.get::<_, Option<String>>("comment")?,
                to_addresses: row.get::<_, String>("to_addresses")?,
                from_address: row.get::<_, String>("from_address")?,
                subject: row.get::<_, String>("subject")?,
                message_id: row.get::<_, String>("message_id")?,
                message: row.get::<_, Vec<u8>>("message")?,
                timestamp: row.get::<_, u64>("timestamp")?,
                datetime: row.get::<_, DateTime>("datetime")?,
            })
        };
        let mut stmt = if index.is_empty() {
            tx.connection
                .prepare("DELETE FROM queue WHERE which = ? RETURNING *;")?
        } else {
            tx.connection
                .prepare("DELETE FROM queue WHERE which = ? AND pk IN rarray(?) RETURNING *;")?
        };
        let iter = if index.is_empty() {
            stmt.query_map([&queue.as_str()], cl)?
        } else {
            // Note: A `Rc<Vec<Value>>` must be used as the parameter.
            let index = std::rc::Rc::new(
                index
                    .into_iter()
                    .map(rusqlite::types::Value::from)
                    .collect::<Vec<rusqlite::types::Value>>(),
            );
            stmt.query_map(rusqlite::params![queue.as_str(), index], cl)?
        };

        let mut ret = vec![];
        for item in iter {
            let item = item?;
            ret.push(item);
        }
        drop(stmt);
        tx.commit()?;
        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn test_queue_delete_array() {
        use tempfile::TempDir;

        let tmp_dir = TempDir::new().unwrap();
        let db_path = tmp_dir.path().join("mpot.db");
        let config = Configuration {
            send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
            db_path,
            data_path: tmp_dir.path().to_path_buf(),
            administrators: vec![],
        };

        let db = Connection::open_or_create_db(config).unwrap().trusted();
        for i in 0..5 {
            db.insert_to_queue(
                QueueEntry::new(
                    Queue::Hold,
                    None,
                    None,
                    format!("Subject: testing\r\nMessage-Id: {i}@localhost\r\n\r\nHello\r\n")
                        .as_bytes(),
                    None,
                )
                .unwrap(),
            )
            .unwrap();
        }
        let entries = db.queue(Queue::Hold).unwrap();
        assert_eq!(entries.len(), 5);
        let out_entries = db.delete_from_queue(Queue::Out, vec![]).unwrap();
        assert_eq!(db.queue(Queue::Hold).unwrap().len(), 5);
        assert!(out_entries.is_empty());
        let deleted_entries = db.delete_from_queue(Queue::Hold, vec![]).unwrap();
        assert_eq!(deleted_entries.len(), 5);
        assert_eq!(
            &entries
                .iter()
                .cloned()
                .map(DbVal::into_inner)
                .map(|mut e| {
                    e.pk = -1;
                    e
                })
                .collect::<Vec<_>>(),
            &deleted_entries
        );

        for e in deleted_entries {
            db.insert_to_queue(e).unwrap();
        }

        let index = db
            .queue(Queue::Hold)
            .unwrap()
            .into_iter()
            .skip(2)
            .map(|e| e.pk())
            .take(2)
            .collect::<Vec<i64>>();
        let deleted_entries = db.delete_from_queue(Queue::Hold, index).unwrap();
        assert_eq!(deleted_entries.len(), 2);
        assert_eq!(db.queue(Queue::Hold).unwrap().len(), 3);
    }
}
