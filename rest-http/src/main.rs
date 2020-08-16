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

#![feature(proc_macro_hygiene, decl_macro)]
extern crate mailpot;

pub use mailpot::config::*;
pub use mailpot::db::*;
pub use mailpot::errors::*;
pub use mailpot::models::*;
pub use mailpot::post::*;
pub use mailpot::*;
use std::path::PathBuf;

use rocket::response::content;
use rocket_contrib::json::Json;

#[macro_use]
extern crate rocket;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/lists")]
fn lists() -> Json<Vec<MailingList>> {
    let db = Database::open_or_create_db().unwrap();
    let lists = db.list_lists().unwrap();
    Json(lists)
}

#[get("/lists/<num>")]
fn lists_num(num: u64) -> Json<Option<MailingList>> {
    let db = Database::open_or_create_db().unwrap();
    let list = db.get_list(num as i64).unwrap();
    Json(list)
}

#[get("/lists/<num>/members")]
fn lists_members(num: u64) -> Option<Json<Vec<ListMembership>>> {
    let db = Database::open_or_create_db().unwrap();
    db.list_members(num as i64).ok().map(|l| Json(l))
}

#[get("/lists/<num>/owners")]
fn lists_owners(num: u64) -> Option<Json<Vec<ListOwner>>> {
    let db = Database::open_or_create_db().unwrap();
    db.get_list_owners(num as i64).ok().map(|l| Json(l))
}

#[post("/lists/<num>/owners/add", data = "<new_owner>")]
fn lists_owner_add(num: u64, new_owner: ListOwner) -> Result<()> {
    todo!()
}

#[get("/lists/<num>/policy")]
fn lists_policy(num: u64) -> Option<Json<Option<PostPolicy>>> {
    let db = Database::open_or_create_db().unwrap();
    db.get_list_policy(num as i64).ok().map(|l| Json(l))
}

fn main() {
    rocket::ignite()
        .mount(
            "/",
            routes![
                index,
                lists_members,
                lists_num,
                lists_owners,
                lists_policy,
                lists
            ],
        )
        .launch();
}
