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
use serde_json::{json, Value};

impl Database {
    pub fn insert_to_error_queue(&self, env: &Envelope, raw: &[u8]) -> Result<i64> {
        let mut stmt = self.connection.prepare("INSERT INTO error_queue(to_address, from_address, subject, message_id, message, timestamp, datetime) VALUES(?, ?, ?, ?, ?, ?, ?) RETURNING pk;")?;
        let pk = stmt.query_row(
            rusqlite::params![
                &env.field_to_to_string(),
                &env.field_from_to_string(),
                &env.subject(),
                &env.message_id().to_string(),
                raw,
                &env.timestamp,
                &env.date,
            ],
            |row| {
                let pk: i64 = row.get("pk")?;
                Ok(pk)
            },
        )?;
        Ok(pk)
    }

    pub fn error_queue(&self) -> Result<Vec<DbVal<Value>>> {
        let mut stmt = self.connection.prepare("SELECT * FROM error_queue;")?;
        let error_iter = stmt.query_map([], |row| {
            let pk = row.get::<_, i64>("pk")?;
            Ok(DbVal(
                json!({
                    "pk" : pk,
                    "to_address": row.get::<_, String>("to_address")?,
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
        for error in error_iter {
            let error = error?;
            ret.push(error);
        }
        Ok(ret)
    }

    pub fn delete_from_error_queue(&mut self, index: Vec<i64>) -> Result<()> {
        let tx = self.connection.transaction()?;

        if index.is_empty() {
            tx.execute("DELETE FROM error_queue;", [])?;
        } else {
            for i in index {
                tx.execute(
                    "DELETE FROM error_queue WHERE pk = ?;",
                    rusqlite::params![i],
                )?;
            }
        };
        tx.commit()?;
        Ok(())
    }
}
