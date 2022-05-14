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

use askama::Template;
use percent_encoding::percent_decode_str;

#[derive(Template)]
#[template(path = "list.html")]
struct ListTemplate<'a> {
    title: &'a str,
    _list: &'a DbVal<MailingList>,
    posts: Vec<DbVal<Post>>,
    body: &'a str,
}

#[derive(Template)]
#[template(path = "post.html")]
struct PostTemplate<'a> {
    title: &'a str,
    _list: &'a DbVal<MailingList>,
    _post: DbVal<Post>,
    body: &'a str,
    _from: &'a str,
    _to: &'a str,
    subject: &'a str,
    _in_reply_to: Option<String>,
    _references: Vec<String>,
}

use warp::Filter;
#[tokio::main]
async fn main() {
    let list_handler = warp::path!("lists" / i64).map(|list_pk: i64| {
        let db = Database::open_or_create_db().unwrap();
        let list = db.get_list(list_pk).unwrap().unwrap();
        let posts = db.list_posts(list_pk, None).unwrap();
        let template = ListTemplate {
            title: &list.name,
            _list: &list,
            posts,
            body: &list.description.clone().unwrap_or_default(),
        };
        let res = template.render().unwrap();
        Ok(warp::reply::html(res))
    });
    let post_handler =
        warp::path!("list" / i64 / String).map(|list_pk: i64, message_id: String| {
            let message_id = percent_decode_str(&message_id).decode_utf8().unwrap();
            let db = Database::open_or_create_db().unwrap();
            let list = db.get_list(list_pk).unwrap().unwrap();
            let posts = db.list_posts(list_pk, None).unwrap();
            let post = posts
                .into_iter()
                .find(|p| message_id.contains(&p.message_id))
                .unwrap();
            let envelope = melib::Envelope::from_bytes(post.message.as_slice(), None)
                .expect("Could not parse mail");
            let body = envelope.body_bytes(post.message.as_slice());
            let body_text = body.text();
            let template = PostTemplate {
                title: &list.name,
                _list: &list,
                _post: post,
                body: &body_text,
                _from: &envelope.field_from_to_string(),
                _to: &envelope.field_to_to_string(),
                subject: &envelope.subject(),
                _in_reply_to: envelope.in_reply_to_display().map(|r| r.to_string()),
                _references: envelope
                    .references()
                    .into_iter()
                    .map(|m| m.to_string())
                    .collect::<Vec<String>>(),
            };
            let res = template.render().unwrap();
            Ok(warp::reply::html(res))
        });
    let routes = warp::get().and(list_handler).or(post_handler);

    // Note that composing filters for many routes may increase compile times (because it uses a lot of generics).
    // If you wish to use dynamic dispatch instead and speed up compile times while
    // making it slightly slower at runtime, you can use Filter::boxed().

    eprintln!("Running at 127.0.0.1:3030");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
