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

//! Utils for templates with the [`minijinja`] crate.

use mailpot::models::{DbVal, ListOwner};
use minijinja::{
    value::{Object, Value},
    Error,
};

use crate::minijinja_utils::topics_common;

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize)]
pub struct MailingList {
    pub pk: i64,
    pub name: String,
    pub id: String,
    pub address: String,
    pub description: Option<String>,
    pub topics: Vec<String>,
    #[serde(serialize_with = "crate::utils::to_safe_string_opt")]
    pub archive_url: Option<String>,
    pub inner: DbVal<mailpot::models::MailingList>,
    #[serde(default)]
    pub is_description_html_safe: bool,
}

impl MailingList {
    /// Set whether it's safe to not escape the list's description field.
    ///
    /// If anyone can display arbitrary html in the server, that's bad.
    ///
    /// Note: uses `Borrow` so that it can use both `DbVal<ListOwner>` and
    /// `ListOwner` slices.
    pub fn set_safety<O: std::borrow::Borrow<ListOwner>>(
        &mut self,
        owners: &[O],
        administrators: &[String],
    ) {
        if owners.is_empty() || administrators.is_empty() {
            return;
        }
        self.is_description_html_safe = owners
            .iter()
            .any(|o| administrators.contains(&o.borrow().address));
    }
}

impl From<DbVal<mailpot::models::MailingList>> for MailingList {
    fn from(val: DbVal<mailpot::models::MailingList>) -> Self {
        let DbVal(
            mailpot::models::MailingList {
                pk,
                name,
                id,
                address,
                description,
                topics,
                archive_url,
            },
            _,
        ) = val.clone();

        Self {
            pk,
            name,
            id,
            address,
            description,
            topics,
            archive_url,
            inner: val,
            is_description_html_safe: false,
        }
    }
}

impl std::fmt::Display for MailingList {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.id.fmt(fmt)
    }
}

impl Object for MailingList {
    fn kind(&self) -> minijinja::value::ObjectKind<'_> {
        minijinja::value::ObjectKind::Struct(self)
    }

    fn call_method(
        &self,
        _state: &minijinja::State,
        name: &str,
        _args: &[Value],
    ) -> std::result::Result<Value, Error> {
        match name {
            "subscription_mailto" => {
                Ok(Value::from_serializable(&self.inner.subscription_mailto()))
            }
            "unsubscription_mailto" => Ok(Value::from_serializable(
                &self.inner.unsubscription_mailto(),
            )),
            "topics" => topics_common(&self.topics),
            _ => Err(Error::new(
                minijinja::ErrorKind::UnknownMethod,
                format!("object has no method named {name}"),
            )),
        }
    }
}

impl minijinja::value::StructObject for MailingList {
    fn get_field(&self, name: &str) -> Option<Value> {
        match name {
            "pk" => Some(Value::from_serializable(&self.pk)),
            "name" => Some(Value::from_serializable(&self.name)),
            "id" => Some(Value::from_serializable(&self.id)),
            "address" => Some(Value::from_serializable(&self.address)),
            "description" if self.is_description_html_safe => {
                self.description.as_ref().map_or_else(
                    || Some(Value::from_serializable(&self.description)),
                    |d| Some(Value::from_safe_string(d.clone())),
                )
            }
            "description" => Some(Value::from_serializable(&self.description)),
            "topics" => Some(Value::from_serializable(&self.topics)),
            "archive_url" => Some(Value::from_serializable(&self.archive_url)),
            "is_description_html_safe" => {
                Some(Value::from_serializable(&self.is_description_html_safe))
            }
            _ => None,
        }
    }

    fn static_fields(&self) -> Option<&'static [&'static str]> {
        Some(
            &[
                "pk",
                "name",
                "id",
                "address",
                "description",
                "topics",
                "archive_url",
                "is_description_html_safe",
            ][..],
        )
    }
}
