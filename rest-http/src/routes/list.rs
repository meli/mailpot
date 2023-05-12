use std::sync::Arc;

pub use axum::extract::{Path, Query, State};
use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use mailpot_web::{typed_paths::*, ResponseError, RouterExt};
use serde::{Deserialize, Serialize};

use crate::*;

pub fn create_route(conf: Arc<Configuration>) -> Router {
    Router::new()
        .route("/list/", get(all_lists))
        .route("/list/", post(post_list))
        .typed_get(get_list)
        .with_state(conf)
}

async fn get_list(
    ListPath(id): ListPath,
    State(state): State<Arc<Configuration>>,
) -> Result<Json<DbVal<MailingList>>, ResponseError> {
    let db = Connection::open_db(Configuration::clone(&state))?;
    let Some(list) = (match id {
        ListPathIdentifier::Pk(id) => db.list(id)?,
        ListPathIdentifier::Id(id) => db.list_by_id(id)?,
    }) else {
        return Err(mailpot_web::ResponseError::new(
            "Not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };
    Ok(Json(list))
}

async fn all_lists(
    Query(GetRequest {
        filter: _,
        count,
        page,
    }): Query<GetRequest>,
    State(state): State<Arc<Configuration>>,
) -> Result<Json<GetResponse>, ResponseError> {
    let db = Connection::open_db(Configuration::clone(&state))?;
    let lists_values = db.lists()?;
    let page = page.unwrap_or(0);
    let Some(count) = count else {
        let mut stmt = db
            .connection
            .prepare("SELECT count(*) FROM list;")?;
        return Ok(Json(GetResponse {
            entries: vec![],
            total: stmt.query_row([], |row| {
            let count: usize = row.get(0)?;
            Ok(count)
        })?,
        start: 0,
        }));
    };
    let offset = page * count;
    let res: Vec<_> = lists_values.into_iter().skip(offset).take(count).collect();

    Ok(Json(GetResponse {
        total: res.len(),
        start: offset,
        entries: res,
    }))
}

async fn post_list(
    State(state): State<Arc<Configuration>>,
    Json(_body): Json<GetRequest>,
) -> Result<Json<()>, ResponseError> {
    let _db = Connection::open_db(Configuration::clone(&state))?;
    //    let password_hash = list::hash_password(body.password).await?;
    //    let list = list::new(body.name, body.email, password_hash);
    //    let list = list::create(list).await?;
    //    let res = Publiclist::from(list);
    //

    Ok(Json(()))
}

#[derive(Debug, Serialize, Deserialize)]
enum GetFilter {
    Pk(i64),
    Address(String),
    Id(String),
    Name(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct GetRequest {
    filter: Option<GetFilter>,
    count: Option<usize>,
    page: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GetResponse {
    entries: Vec<DbVal<MailingList>>,
    total: usize,
    start: usize,
}
