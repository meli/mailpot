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

pub use mailpot::config::*;
pub use mailpot::db::*;
pub use mailpot::errors::*;
pub use mailpot::models::*;
pub use mailpot::*;

use std::fs::OpenOptions;
use std::io::Write;

use minijinja::{Environment, Source};

lazy_static::lazy_static! {
    pub static ref TEMPLATES: Environment<'static> = {
        let mut env = Environment::new();
        env.set_source(Source::from_path("src/templates/"));

        env
    };
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
                    list => &list,
                    posts => &posts,
                    months => &months,
                    body => &list.description.as_deref().unwrap_or_default(),
                    root_prefix => &root_url_prefix,
                }
            })
            .collect::<Vec<_>>();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
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
    dbg!(&lists_values);
    for list in &lists_values {
        lists_path.push("lists");
        lists_path.push(list.pk.to_string());
        std::fs::create_dir_all(&lists_path)?;
        lists_path.push("index.html");

        let list = db
            .get_list(list.pk)?
            .ok_or_else(|| format!("List with pk {} not found in database", list.pk))?;
        let months = db.months(list.pk)?;
        let posts = db.list_posts(list.pk, None)?;
        let posts_ctx = posts
            .iter()
            .map(|post| {
                let envelope = melib::Envelope::from_bytes(post.message.as_slice(), None)
                    .expect("Could not parse mail");
                let mut msg_id = &post.message_id[1..];
                msg_id = &msg_id[..msg_id.len().saturating_sub(1)];
                minijinja::context! {
                        pk => post.pk,
                        list => post.list,
                        subject => envelope.subject(),
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
            list=> &list,
            months=> &months,
            posts=> posts_ctx,
            body=>&list.description.clone().unwrap_or_default(),
            root_prefix => &root_url_prefix,
        };
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
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
                in_reply_to => &envelope.in_reply_to_display().map(|r| r.to_string()),
                references => &envelope .references() .into_iter() .map(|m| m.to_string()) .collect::<Vec<String>>(),
                    root_prefix => &root_url_prefix,
            };
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
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
