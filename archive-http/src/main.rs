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

use chrono::Datelike;

mod cal;
mod utils;

use std::borrow::Cow;

pub use mailpot::{models::DbVal, *};
use minijinja::{
    value::{Object, Value},
    Environment, Error, Source, State,
};
use percent_encoding::percent_decode_str;
use utils::*;
use warp::Filter;

#[tokio::main]
async fn main() {
    let config_path = std::env::args()
        .nth(1)
        .expect("Expected configuration file path as first argument.");
    let conf = Configuration::from_file(config_path).unwrap();

    let conf1 = conf.clone();
    let list_handler = warp::path!("lists" / i64).map(move |list_pk: i64| {
        let db = Connection::open_db(conf1.clone()).unwrap();
        let list = db.list(list_pk).unwrap().unwrap();
        let post_policy = db.list_post_policy(list.pk).unwrap();
        let months = db.months(list.pk).unwrap();
        let posts = db.list_posts(list.pk, None).unwrap();
        let mut hist = months
            .iter()
            .map(|m| (m.to_string(), [0usize; 31]))
            .collect::<std::collections::HashMap<String, [usize; 31]>>();
        let posts_ctx = posts
            .iter()
            .map(|post| {
                //2019-07-14T14:21:02
                if let Some(day) = post.datetime.get(8..10).and_then(|d| d.parse::<u64>().ok()) {
                    hist.get_mut(&post.month_year).unwrap()[day.saturating_sub(1) as usize] += 1;
                }
                let envelope = melib::Envelope::from_bytes(post.message.as_slice(), None)
                    .expect("Could not parse mail");
                let mut msg_id = &post.message_id[1..];
                msg_id = &msg_id[..msg_id.len().saturating_sub(1)];
                let subject = envelope.subject();
                let mut subject_ref = subject.trim();
                if subject_ref.starts_with('[')
                    && subject_ref[1..].starts_with(&list.id)
                    && subject_ref[1 + list.id.len()..].starts_with(']')
                {
                    subject_ref = subject_ref[2 + list.id.len()..].trim();
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
                    root_prefix => "",
                }
            })
            .collect::<Vec<_>>();
        let crumbs = vec![
            Crumb {
                label: "Lists".into(),
                url: "/".into(),
            },
            Crumb {
                label: list.name.clone().into(),
                url: format!("/lists/{}/", list.pk).into(),
            },
        ];
        let context = minijinja::context! {
            title=> &list.name,
            description=> &list.description,
            post_policy=> &post_policy,
            preamble => true,
            months=> &months,
            hists => &hist,
            posts=> posts_ctx,
            body=>&list.description.clone().unwrap_or_default(),
            root_prefix => "",
            list => Value::from_object(MailingList::from(list)),
            crumbs => crumbs,
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
            dbg!(&message_id);
            let db = Connection::open_db(conf2.clone()).unwrap();
            let list = db.list(list_pk).unwrap().unwrap();
            let posts = db.list_posts(list_pk, None).unwrap();
            let post = posts
                .iter()
                .find(|p| message_id.contains(p.message_id.as_str().strip_carets()))
                .unwrap();
            //let mut msg_id = &post.message_id[1..];
            //msg_id = &msg_id[..msg_id.len().saturating_sub(1)];
            let envelope = melib::Envelope::from_bytes(post.message.as_slice(), None)
                .map_err(|err| format!("Could not parse mail {}: {err}", post.message_id)).unwrap();
            let body = envelope.body_bytes(post.message.as_slice());
            let body_text = body.text();
            let subject = envelope.subject();
            let mut subject_ref = subject.trim();
            if subject_ref.starts_with('[')
                && subject_ref[1..].starts_with(&list.id)
                && subject_ref[1 + list.id.len()..].starts_with(']')
            {
                subject_ref = subject_ref[2 + list.id.len()..].trim();
            }
            let mut message_id = &post.message_id[1..];
            message_id = &message_id[..message_id.len().saturating_sub(1)];
            let crumbs = vec![
                Crumb {
                    label: "Lists".into(),
                    url: "/".into(),
                },
                Crumb {
                    label: list.name.clone().into(),
                    url: format!("/lists/{}/", list.pk).into(),
                },
                Crumb {
                    label: subject_ref.to_string().into(),
                    url: format!("/lists/{}/{message_id}.html/", list.pk).into(),
                },
            ];
            let context = minijinja::context! {
                title => &list.name,
                list => &list,
                post => &post,
                body => &body_text,
                from => &envelope.field_from_to_string(),
                date => &envelope.date_as_str(),
                to => &envelope.field_to_to_string(),
                subject => &envelope.subject(),
                trimmed_subject => subject_ref,
                in_reply_to => &envelope.in_reply_to_display().map(|r| r.to_string().as_str().strip_carets().to_string()),
                references => &envelope .references() .into_iter() .map(|m| m.to_string().as_str().strip_carets().to_string()) .collect::<Vec<String>>(),
                root_prefix => "",
                crumbs => crumbs,
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
        let db = Connection::open_db(conf3.clone()).unwrap();
        let lists_values = db.lists().unwrap();
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
                    root_prefix => "",
                    list => Value::from_object(MailingList::from(list.clone())),
                }
            })
            .collect::<Vec<_>>();
        let crumbs = vec![Crumb {
            label: "Lists".into(),
            url: "/".into(),
        }];

        let context = minijinja::context! {
            title => "mailing list archive",
            description => "",
            lists => &lists,
            root_prefix => "",
            crumbs => crumbs,
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

    // Note that composing filters for many routes may increase compile times
    // (because it uses a lot of generics). If you wish to use dynamic dispatch
    // instead and speed up compile times while making it slightly slower at
    // runtime, you can use Filter::boxed().

    eprintln!("Running at http://127.0.0.1:3030");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
