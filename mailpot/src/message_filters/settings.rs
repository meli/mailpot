/*
 * This file is part of mailpot
 *
 * Copyright 2023 - Manos Pitsidianakis
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

//! Message filter settings.

use std::collections::HashMap;

use serde_json::Value;

use crate::{errors::*, Connection, DbVal};

impl Connection {
    /// Get message filter settings values for a specific list.
    pub fn get_settings(&self, list_pk: i64) -> Result<HashMap<String, DbVal<Value>>> {
        let mut stmt = self.connection.prepare(
            "SELECT pk, name, value FROM list_settings_json WHERE list = ? AND is_valid = 1;",
        )?;
        let iter = stmt.query_map(rusqlite::params![&list_pk], |row| {
            let pk: i64 = row.get("pk")?;
            let name: String = row.get("name")?;
            let value: Value = row.get("value")?;
            Ok((name, DbVal(value, pk)))
        })?;
        Ok(iter.collect::<std::result::Result<HashMap<String, DbVal<Value>>, rusqlite::Error>>()?)
    }

    /// Set message filter settings value for a specific list.
    pub fn set_settings(&self, list_pk: i64, name: &str, value: Value) -> Result<()> {
        let mut stmt = self.connection.prepare(
            "INSERT OR REPLACE INTO list_settings_json(name, list, value) VALUES(?, ?, ?) \
             RETURNING pk, value;",
        )?;
        stmt.query_row(rusqlite::params![name, &list_pk, &value], |row| {
            let _pk: i64 = row.get("pk")?;
            Ok(())
        })?;
        Ok(())
    }
}
