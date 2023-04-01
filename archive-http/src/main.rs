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

use std::sync::Arc;

use minijinja::{Environment, Source};
use percent_encoding::percent_decode_str;
use warp::Filter;

lazy_static::lazy_static! {
    pub static ref TEMPLATES: Environment<'static> = {
        let mut env = Environment::new();
        env.set_source(Source::from_path("src/templates/"));

        env
    };
}

#[tokio::main]
async fn main() {
    let config_path = std::env::args()
        .nth(1)
        .expect("Expected configuration file path as first argument.");
    let conf = Arc::new(Configuration::from_file(config_path).unwrap());

    let conf1 = conf.clone();
    let list_handler = warp::path!("lists" / i64).map(move |list_pk: i64| {
        let db = Database::open_db(&conf1).unwrap();
        let list = db.get_list(list_pk).unwrap().unwrap();
        let months = db.months(list_pk).unwrap();
        let posts = db
            .list_posts(list_pk, None)
            .unwrap()
            .into_iter()
            .map(|post| {
                let envelope = melib::Envelope::from_bytes(post.message.as_slice(), None)
                    .expect("Could not parse mail");
                minijinja::context! {
                        pk => post.pk,
                        list => post.list,
                        subject => envelope.subject(),
                        address=> post.address,
                        message_id => post.message_id,
                        message => post.message,
                        timestamp => post.timestamp,
                        datetime => post.datetime,
                }
            })
            .collect::<Vec<_>>();
        let context = minijinja::context! {
            title=> &list.name,
            list=> &list,
            months=> &months,
            posts=> posts,
            body=>&list.description.clone().unwrap_or_default(),
        };
        Ok(warp::reply::html(
            TEMPLATES
                .get_template("list.html")
                .unwrap()
                .render(context)
                .unwrap_or_else(|err| err.to_string()),
        ))
    });
    let conf2 = conf.clone();
    let post_handler =
        warp::path!("list" / i64 / String).map(move |list_pk: i64, message_id: String| {
            let message_id = percent_decode_str(&message_id).decode_utf8().unwrap();
            let db = Database::open_db(&conf2).unwrap();
            let list = db.get_list(list_pk).unwrap().unwrap();
            let posts = db.list_posts(list_pk, None).unwrap();
            let post = posts
                .iter()
                .find(|p| message_id.contains(&p.message_id))
                .unwrap();
            let envelope = melib::Envelope::from_bytes(post.message.as_slice(), None)
                .expect("Could not parse mail");
            let body = envelope.body_bytes(post.message.as_slice());
            let body_text = body.text();
            let context = minijinja::context !{
                title => &list.name,
                list => &list,
                post => &post,
                posts => &posts,
                body => &body_text,
                from => &envelope.field_from_to_string(),
                date => &envelope.date_as_str(),
                to => &envelope.field_to_to_string(),
                subject => &envelope.subject(),
                in_reply_to => &envelope.in_reply_to_display().map(|r| r.to_string()),
                references => &envelope .references() .into_iter() .map(|m| m.to_string()) .collect::<Vec<String>>(),
            };
            Ok(warp::reply::html(
                    TEMPLATES
                    .get_template("post.html")
                    .unwrap()
                    .render(context)
                .unwrap_or_else(|err| err.to_string()),
            ))
        });
    let conf3 = conf.clone();
    let index_handler = warp::path::end().map(move || {
        let db = Database::open_db(&conf3).unwrap();
        let lists_values = db.list_lists().unwrap();
        dbg!(&lists_values);
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
                }
            })
            .collect::<Vec<_>>();
        let context = minijinja::context! {
            title => "mailing list archive",
            description => "",
            lists => &lists,
        };
        Ok(warp::reply::html(
            TEMPLATES
                .get_template("lists.html")
                .unwrap()
                .render(context)
                .unwrap_or_else(|err| err.to_string()),
        ))
    });
    let routes = warp::get()
        .and(index_handler)
        .or(list_handler)
        .or(post_handler);

    // Note that composing filters for many routes may increase compile times (because it uses a lot of generics).
    // If you wish to use dynamic dispatch instead and speed up compile times while
    // making it slightly slower at runtime, you can use Filter::boxed().

    eprintln!("Running at http://127.0.0.1:3030");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
