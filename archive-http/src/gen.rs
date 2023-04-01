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

extern crate mailpot;
use chrono::Datelike;

mod cal;

pub use mailpot::config::*;
pub use mailpot::db::*;
pub use mailpot::errors::*;
pub use mailpot::models::*;
pub use mailpot::*;

use std::fs::OpenOptions;
use std::io::Write;

use minijinja::{Environment, Error, Source, State};

use minijinja::value::{from_args, Object, SeqObject, Value};

lazy_static::lazy_static! {
    pub static ref TEMPLATES: Environment<'static> = {
        let mut env = Environment::new();
        env.add_function("calendarize", calendarize);
        env.set_source(Source::from_path("src/templates/"));

        env
    };
}

trait StripCarets {
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
            "subscribe_mailto" => Ok(Value::from_serializable(&self.inner.subscribe_mailto())),
            "unsubscribe_mailto" => Ok(Value::from_serializable(&self.inner.unsubscribe_mailto())),
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

fn calendarize(_state: &State, args: Value, hists: Value) -> std::result::Result<Value, Error> {
    use chrono::Month;
    use std::convert::TryFrom;

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

fn run_app() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let Some(config_path) = args
        .get(1) else {
        return Err("Expected configuration file path as first argument.".into());
        };
    let Some(output_path) = args
        .get(2) else {
        return Err("Expected output dir path as second argument.".into());
        };
    let root_url_prefix = args.get(3).cloned().unwrap_or_default();

    let output_path = std::path::Path::new(&output_path);
    if output_path.exists() && !output_path.is_dir() {
        return Err("Output path is not a directory.".into());
    }

    std::fs::create_dir_all(&output_path.join("lists"))?;
    std::fs::create_dir_all(&output_path.join("list"))?;
    let conf = Configuration::from_file(config_path)
        .map_err(|err| format!("Could not load config {config_path}: {err}"))?;

    let db = Database::open_db(&conf).map_err(|err| format!("Couldn't open db: {err}"))?;
    let lists_values = db.list_lists()?;
    {
        //index.html

        let lists = lists_values
            .iter()
            .map(|list| {
                let months = db.months(list.pk).unwrap();
                let posts = db.list_posts(list.pk, None).unwrap();
                minijinja::context! {
                    title => &list.name,
                    posts => &posts,
                    months => &months,
                    body => &list.description.as_deref().unwrap_or_default(),
                    root_prefix => &root_url_prefix,
                    list => Value::from_object(MailingList::from(list.clone())),
                }
            })
            .collect::<Vec<_>>();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&output_path.join("index.html"))?;

        let context = minijinja::context! {
            title => "mailing list archive",
            description => "",
            lists => &lists,
                    root_prefix => &root_url_prefix,
        };
        file.write_all(
            TEMPLATES
                .get_template("lists.html")?
                .render(context)?
                .as_bytes(),
        )?;
    }

    let mut lists_path = output_path.to_path_buf();

    for list in &lists_values {
        lists_path.push("lists");
        lists_path.push(list.pk.to_string());
        std::fs::create_dir_all(&lists_path)?;
        lists_path.push("index.html");

        let list = db
            .get_list(list.pk)?
            .ok_or_else(|| format!("List with pk {} not found in database", list.pk))?;
        let post_policy = db.get_list_policy(list.pk)?;
        let months = db.months(list.pk)?;
        let posts = db.list_posts(list.pk, None)?;
        let mut hist = months
            .iter()
            .map(|m| (m.to_string(), [0usize; 31]))
            .collect::<std::collections::HashMap<String, [usize; 31]>>();
        let posts_ctx = posts
            .iter()
            .map(|post| {
                //2019-07-14T14:21:02
                if let (Some(month), Some(day)) = (
                    post.datetime.get(5..7).and_then(|m| m.parse::<u64>().ok()),
                    post.datetime.get(8..10).and_then(|d| d.parse::<u64>().ok()),
                ) {
                    hist.get_mut(&post.month_year).unwrap()[day.saturating_sub(1) as usize] += 1;
                }
                let envelope = melib::Envelope::from_bytes(post.message.as_slice(), None)
                    .expect("Could not parse mail");
                let mut msg_id = &post.message_id[1..];
                msg_id = &msg_id[..msg_id.len().saturating_sub(1)];
                let subject = envelope.subject();
                let mut subject_ref = subject.trim();
                if subject_ref.starts_with('[') {
                    if subject_ref[1..].starts_with(&list.id)
                        && subject_ref[1 + list.id.len()..].starts_with(']')
                    {
                        subject_ref = subject_ref[2 + list.id.len()..].trim();
                    }
                }
                minijinja::context! {
                        pk => post.pk,
                        list => post.list,
                        subject => subject_ref,
                        address=> post.address,
                        message_id => msg_id,
                        message => post.message,
                        timestamp => post.timestamp,
                        datetime => post.datetime,
                    root_prefix => &root_url_prefix,
                }
            })
            .collect::<Vec<_>>();
        let context = minijinja::context! {
            title=> &list.name,
            description=> &list.description,
            post_policy=> &post_policy,
            preamble => true,
            months=> &months,
            hists => &hist,
            posts=> posts_ctx,
            body=>&list.description.clone().unwrap_or_default(),
            root_prefix => &root_url_prefix,
            list => Value::from_object(MailingList::from(list.clone())),
        };
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&lists_path)
            .map_err(|err| format!("could not open {lists_path:?}: {err}"))?;
        file.write_all(
            TEMPLATES
                .get_template("list.html")?
                .render(context)?
                .as_bytes(),
        )?;
        lists_path.pop();
        lists_path.pop();
        lists_path.pop();
        lists_path.push("list");
        lists_path.push(list.pk.to_string());
        std::fs::create_dir_all(&lists_path)?;

        for post in posts {
            let mut msg_id = &post.message_id[1..];
            msg_id = &msg_id[..msg_id.len().saturating_sub(1)];
            lists_path.push(format!("{msg_id}.html"));
            let envelope = melib::Envelope::from_bytes(post.message.as_slice(), None)
                .map_err(|err| format!("Could not parse mail {}: {err}", post.message_id))?;
            let body = envelope.body_bytes(post.message.as_slice());
            let body_text = body.text();
            let subject = envelope.subject();
            let mut subject_ref = subject.trim();
            if subject_ref.starts_with('[') {
                if subject_ref[1..].starts_with(&list.id)
                    && subject_ref[1 + list.id.len()..].starts_with(']')
                {
                    subject_ref = subject_ref[2 + list.id.len()..].trim();
                }
            }
            let context = minijinja::context! {
                title => &list.name,
                list => &list,
                post => &post,
                posts => &posts_ctx,
                body => &body_text,
                from => &envelope.field_from_to_string(),
                date => &envelope.date_as_str(),
                to => &envelope.field_to_to_string(),
                subject => &envelope.subject(),
                trimmed_subject => subject_ref,
                in_reply_to => &envelope.in_reply_to_display().map(|r| r.to_string().as_str().strip_carets().to_string()),
                references => &envelope .references() .into_iter() .map(|m| m.to_string().as_str().strip_carets().to_string()) .collect::<Vec<String>>(),
                    root_prefix => &root_url_prefix,
            };
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&lists_path)
                .map_err(|err| format!("could not open {lists_path:?}: {err}"))?;
            file.write_all(
                TEMPLATES
                    .get_template("post.html")?
                    .render(context)?
                    .as_bytes(),
            )?;
            lists_path.pop();
        }
        lists_path.pop();
        lists_path.pop();
    }
    Ok(())
}

fn main() -> std::result::Result<(), i64> {
    if let Err(err) = run_app() {
        eprintln!("{err}");
        return Err(-1);
    }
    Ok(())
}
