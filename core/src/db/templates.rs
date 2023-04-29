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

//! Named templates, for generated e-mail like confirmations, alerts etc.

use super::*;

impl Connection {
    /// Fetch all.
    pub fn fetch_templates(&self) -> Result<Vec<DbVal<Template>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM templates ORDER BY pk;")?;
        let iter = stmt.query_map(rusqlite::params![], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                Template {
                    pk,
                    name: row.get("name")?,
                    list: row.get("list")?,
                    subject: row.get("subject")?,
                    headers_json: row.get("headers_json")?,
                    body: row.get("body")?,
                },
                pk,
            ))
        })?;

        let mut ret = vec![];
        for templ in iter {
            let templ = templ?;
            ret.push(templ);
        }
        Ok(ret)
    }

    /// Fetch a named template.
    pub fn fetch_template(
        &self,
        template: &str,
        list_pk: Option<i64>,
    ) -> Result<Option<DbVal<Template>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM templates WHERE name = ? AND list IS ?;")?;
        let ret = stmt
            .query_row(rusqlite::params![&template, &list_pk], |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    Template {
                        pk,
                        name: row.get("name")?,
                        list: row.get("list")?,
                        subject: row.get("subject")?,
                        headers_json: row.get("headers_json")?,
                        body: row.get("body")?,
                    },
                    pk,
                ))
            })
            .optional()?;
        if ret.is_none() && list_pk.is_some() {
            let mut stmt = self
                .connection
                .prepare("SELECT * FROM templates WHERE name = ? AND list IS NULL;")?;
            Ok(stmt
                .query_row(rusqlite::params![&template], |row| {
                    let pk = row.get("pk")?;
                    Ok(DbVal(
                        Template {
                            pk,
                            name: row.get("name")?,
                            list: row.get("list")?,
                            subject: row.get("subject")?,
                            headers_json: row.get("headers_json")?,
                            body: row.get("body")?,
                        },
                        pk,
                    ))
                })
                .optional()?)
        } else {
            Ok(ret)
        }
    }

    /// Insert a named template.
    pub fn add_template(&self, template: Template) -> Result<DbVal<Template>> {
        let mut stmt = self.connection.prepare(
            "INSERT INTO templates(name, list, subject, headers_json, body) VALUES(?, ?, ?, ?, ?) \
             RETURNING *;",
        )?;
        let ret = stmt
            .query_row(
                rusqlite::params![
                    &template.name,
                    &template.list,
                    &template.subject,
                    &template.headers_json,
                    &template.body
                ],
                |row| {
                    let pk = row.get("pk")?;
                    Ok(DbVal(
                        Template {
                            pk,
                            name: row.get("name")?,
                            list: row.get("list")?,
                            subject: row.get("subject")?,
                            headers_json: row.get("headers_json")?,
                            body: row.get("body")?,
                        },
                        pk,
                    ))
                },
            )
            .map_err(|err| {
                if matches!(
                    err,
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error {
                            code: rusqlite::ffi::ErrorCode::ConstraintViolation,
                            extended_code: 787
                        },
                        _
                    )
                ) {
                    Error::from(err).chain_err(|| NotFound("Could not find a list with this pk."))
                } else {
                    err.into()
                }
            })?;

        trace!("add_template {:?}.", &ret);
        Ok(ret)
    }

    /// Remove a named template.
    pub fn remove_template(&self, template: &str, list_pk: Option<i64>) -> Result<Template> {
        let mut stmt = self
            .connection
            .prepare("DELETE FROM templates WHERE name = ? AND list IS ? RETURNING *;")?;
        let ret = stmt.query_row(rusqlite::params![&template, &list_pk], |row| {
            Ok(Template {
                pk: -1,
                name: row.get("name")?,
                list: row.get("list")?,
                subject: row.get("subject")?,
                headers_json: row.get("headers_json")?,
                body: row.get("body")?,
            })
        })?;

        trace!(
            "remove_template {} list_pk {:?} {:?}.",
            template,
            &list_pk,
            &ret
        );
        Ok(ret)
    }
}
