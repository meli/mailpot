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

use serde_json::{json, Value};

use super::*;

/// In-database queues of mail.
#[derive(Copy, Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Queue {
    /// Messages that have been submitted but not yet processed, await
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
}

impl Queue {
    /// Returns the name of the queue used in the database schema.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Maildrop => "maildrop",
            Self::Hold => "hold",
            Self::Deferred => "deferred",
            Self::Corrupt => "corrupt",
            Self::Out => "out",
        }
    }
}

impl Connection {
    /// Insert a received email into a queue.
    pub fn insert_to_queue(
        &self,
        queue: Queue,
        list_pk: Option<i64>,
        env: Option<Cow<'_, Envelope>>,
        raw: &[u8],
        comment: String,
    ) -> Result<i64> {
        let env = env
            .map(Ok)
            .unwrap_or_else(|| melib::Envelope::from_bytes(raw, None).map(Cow::Owned))?;
        let mut stmt = self.connection.prepare(
            "INSERT INTO queue(which, list, comment, to_addresses, from_address, subject, \
             message_id, message) VALUES(?, ?, ?, ?, ?, ?, ?, ?) RETURNING pk;",
        )?;
        let pk = stmt.query_row(
            rusqlite::params![
                queue.as_str(),
                &list_pk,
                &comment,
                &env.field_to_to_string(),
                &env.field_from_to_string(),
                &env.subject(),
                &env.message_id().to_string(),
                raw,
            ],
            |row| {
                let pk: i64 = row.get("pk")?;
                Ok(pk)
            },
        )?;
        Ok(pk)
    }

    /// Fetch all queue entries.
    pub fn queue(&self, queue: Queue) -> Result<Vec<DbVal<Value>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM queue WHERE which = ?;")?;
        let iter = stmt.query_map([&queue.as_str()], |row| {
            let pk = row.get::<_, i64>("pk")?;
            Ok(DbVal(
                json!({
                    "pk" : pk,
                    "comment": row.get::<_, Option<String>>("comment")?,
                    "to_addresses": row.get::<_, String>("to_addresses")?,
                    "from_address": row.get::<_, String>("from_address")?,
                    "subject": row.get::<_, String>("subject")?,
                    "message_id": row.get::<_, String>("message_id")?,
                    "message": row.get::<_, Vec<u8>>("message")?,
                    "timestamp": row.get::<_, u64>("timestamp")?,
                    "datetime": row.get::<_, String>("datetime")?,
                }),
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
    pub fn delete_from_queue(&mut self, queue: Queue, index: Vec<i64>) -> Result<Vec<Value>> {
        let tx = self.connection.transaction()?;

        let cl = |row: &rusqlite::Row<'_>| {
            Ok(json!({
                "pk" : -1,
                "comment": row.get::<_, Option<String>>("comment")?,
                "to_addresses": row.get::<_, String>("to_addresses")?,
                "from_address": row.get::<_, String>("from_address")?,
                "subject": row.get::<_, String>("subject")?,
                "message_id": row.get::<_, String>("message_id")?,
                "message": row.get::<_, Vec<u8>>("message")?,
                "timestamp": row.get::<_, u64>("timestamp")?,
                "datetime": row.get::<_, String>("datetime")?,
            }))
        };
        let mut stmt = if index.is_empty() {
            tx.prepare("DELETE FROM queue WHERE which = ? RETURNING *;")?
        } else {
            tx.prepare("DELETE FROM queue WHERE which = ? AND pk IN rarray(?) RETURNING *;")?
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
