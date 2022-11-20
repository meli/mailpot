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

use warp::Filter;

/*
fn json_body() -> impl Filter<Extract = (String,), Error = warp::Rejection> + Clone {
    // When accepting a body, we want a JSON body
    // (and to reject huge payloads)...
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}
*/

#[tokio::main]
async fn main() {
    let config_path = std::env::args()
        .skip(1)
        .next()
        .expect("Expected configuration file path as first argument.");
    Configuration::init(config_path).unwrap();

    // GET /lists/:i64/policy
    let policy = warp::path!("lists" / i64 / "policy").map(|list_pk| {
        let db = Database::open_or_create_db(&Configuration::db_path().unwrap()).unwrap();
        db.get_list_policy(list_pk)
            .ok()
            .map(|l| warp::reply::json(&l.unwrap()))
            .unwrap()
    });

    //get("/lists")]
    let lists = warp::path!("lists").map(|| {
        let db = Database::open_or_create_db(&Configuration::db_path().unwrap()).unwrap();
        let lists = db.list_lists().unwrap();
        warp::reply::json(&lists)
    });

    //get("/lists/<num>")]
    let lists_num = warp::path!("lists" / i64).map(|list_pk| {
        let db = Database::open_or_create_db(&Configuration::db_path().unwrap()).unwrap();
        let list = db.get_list(list_pk).unwrap();
        warp::reply::json(&list)
    });

    //get("/lists/<num>/members")]
    let lists_members = warp::path!("lists" / i64 / "members").map(|list_pk| {
        let db = Database::open_or_create_db(&Configuration::db_path().unwrap()).unwrap();
        db.list_members(list_pk)
            .ok()
            .map(|l| warp::reply::json(&l))
            .unwrap()
    });

    //get("/lists/<num>/owners")]
    let lists_owners = warp::path!("lists" / i64 / "owners").map(|list_pk| {
        let db = Database::open_or_create_db(&Configuration::db_path().unwrap()).unwrap();
        db.get_list_owners(list_pk)
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
            .or(lists_members)
            .or(lists_owners)
            .or(lists_owner_add),
    );

    // Note that composing filters for many routes may increase compile times (because it uses a lot of generics).
    // If you wish to use dynamic dispatch instead and speed up compile times while
    // making it slightly slower at runtime, you can use Filter::boxed().

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
