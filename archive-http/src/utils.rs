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

lazy_static::lazy_static! {
    pub static ref TEMPLATES: Environment<'static> = {
        let mut env = Environment::new();
        env.add_function("calendarize", calendarize);
        env.set_source(Source::from_path("src/templates/"));

        env
    };
}

pub trait StripCarets {
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

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize)]
pub struct MailingList {
    pub pk: i64,
    pub name: String,
    pub id: String,
    pub address: String,
    pub description: Option<String>,
    pub archive_url: Option<String>,
    pub inner: DbVal<mailpot::models::MailingList>,
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
            archive_url,
            inner: val,
        }
    }
}

impl std::fmt::Display for MailingList {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.id.fmt(fmt)
    }
}

impl Object for MailingList {
    fn kind(&self) -> minijinja::value::ObjectKind {
        minijinja::value::ObjectKind::Struct(self)
    }

    fn call_method(
        &self,
        _state: &State,
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
            _ => Err(Error::new(
                minijinja::ErrorKind::UnknownMethod,
                format!("aaaobject has no method named {name}"),
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
            "description" => Some(Value::from_serializable(&self.description)),
            "archive_url" => Some(Value::from_serializable(&self.archive_url)),
            _ => None,
        }
    }

    fn static_fields(&self) -> Option<&'static [&'static str]> {
        Some(&["pk", "name", "id", "address", "description", "archive_url"][..])
    }
}

pub fn calendarize(_state: &State, args: Value, hists: Value) -> std::result::Result<Value, Error> {
    use chrono::Month;

    macro_rules! month {
        ($int:expr) => {{
            let int = $int;
            match int {
                1 => Month::January.name(),
                2 => Month::February.name(),
                3 => Month::March.name(),
                4 => Month::April.name(),
                5 => Month::May.name(),
                6 => Month::June.name(),
                7 => Month::July.name(),
                8 => Month::August.name(),
                9 => Month::September.name(),
                10 => Month::October.name(),
                11 => Month::November.name(),
                12 => Month::December.name(),
                _ => unreachable!(),
            }
        }};
    }
    let month = args.as_str().unwrap();
    let hist = hists
        .get_item(&Value::from(month))?
        .as_seq()
        .unwrap()
        .iter()
        .map(|v| usize::try_from(v).unwrap())
        .collect::<Vec<usize>>();
    let sum: usize = hists
        .get_item(&Value::from(month))?
        .as_seq()
        .unwrap()
        .iter()
        .map(|v| usize::try_from(v).unwrap())
        .sum();
    let date = chrono::NaiveDate::parse_from_str(&format!("{}-01", month), "%F").unwrap();
    // Week = [Mon, Tue, Wed, Thu, Fri, Sat, Sun]
    Ok(minijinja::context! {
        month_name => month!(date.month()),
        month => month,
        month_int => date.month() as usize,
        year => date.year(),
        weeks => cal::calendarize_with_offset(date, 1),
        hist => hist,
        sum => sum,
    })
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize)]
pub struct Crumb {
    pub label: Cow<'static, str>,
    pub url: Cow<'static, str>,
}
