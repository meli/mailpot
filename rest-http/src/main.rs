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

pub use mailpot::{models::*, *};
use warp::Filter;

#[tokio::main]
async fn main() {
    let config_path = std::env::args()
        .nth(1)
        .expect("Expected configuration file path as first argument.");
    let conf = Configuration::from_file(config_path).unwrap();

    let conf1 = conf.clone();
    // GET /lists/:i64/policy
    let policy = warp::path!("lists" / i64 / "policy").map(move |list_pk| {
        let db = Connection::open_db(conf1.clone()).unwrap();
        db.list_post_policy(list_pk)
            .ok()
            .map(|l| warp::reply::json(&l.unwrap()))
            .unwrap()
    });

    let conf2 = conf.clone();
    //get("/lists")]
    let lists = warp::path!("lists").map(move || {
        let db = Connection::open_db(conf2.clone()).unwrap();
        let lists = db.lists().unwrap();
        warp::reply::json(&lists)
    });

    let conf3 = conf.clone();
    //get("/lists/<num>")]
    let lists_num = warp::path!("lists" / i64).map(move |list_pk| {
        let db = Connection::open_db(conf3.clone()).unwrap();
        let list = db.list(list_pk).unwrap();
        warp::reply::json(&list)
    });

    let conf4 = conf.clone();
    //get("/lists/<num>/subscriptions")]
    let lists_subscriptions = warp::path!("lists" / i64 / "subscriptions").map(move |list_pk| {
        let db = Connection::open_db(conf4.clone()).unwrap();
        db.list_subscriptions(list_pk)
            .ok()
            .map(|l| warp::reply::json(&l))
            .unwrap()
    });

    //get("/lists/<num>/owners")]
    let lists_owners = warp::path!("lists" / i64 / "owners").map(move |list_pk| {
        let db = Connection::open_db(conf.clone()).unwrap();
        db.list_owners(list_pk)
            .ok()
            .map(|l| warp::reply::json(&l))
            .unwrap()
    });

    //post("/lists/<num>/owners/add", data = "<new_owner>")]
    let lists_owner_add =
        warp::post().and(warp::path!("lists" / i64 / "owners" / "add").map(|_list_pk| "todo"));

    let routes = warp::get().and(
        lists
            .or(policy)
            .or(lists_num)
            .or(lists_subscriptions)
            .or(lists_owners)
            .or(lists_owner_add),
    );

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
