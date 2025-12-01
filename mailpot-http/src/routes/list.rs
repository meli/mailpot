use std::sync::Arc;

pub use axum::extract::{Path, Query, State};
use axum::{http::StatusCode, Json, Router};
use mailpot_web::{typed_paths::*, ResponseError, RouterExt, TypedPath};
use serde::{Deserialize, Serialize};

use crate::*;

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/list/")]
pub struct ListsPath;

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/list/:id/owner/")]
pub struct ListOwnerPath(pub ListPathIdentifier);

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/list/:id/subscription/")]
pub struct ListSubscriptionPath(pub ListPathIdentifier);

pub fn create_route(conf: Arc<Configuration>) -> Router {
    Router::new()
        .typed_get(all_lists)
        .typed_post(new_list)
        .typed_get(get_list)
        .typed_post({
            move |_: ListPath| async move {
                Err::<(), ResponseError>(mailpot_web::ResponseError::new(
                    "Invalid method".to_string(),
                    StatusCode::BAD_REQUEST,
                ))
            }
        })
        .typed_get(get_list_owner)
        .typed_post(new_list_owner)
        .typed_get(get_list_subs)
        .typed_post(new_list_sub)
        .with_state(conf)
}

async fn get_list(
    ListPath(id): ListPath,
    State(state): State<Arc<Configuration>>,
) -> Result<Json<MailingList>, ResponseError> {
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
    Ok(Json(list.into_inner()))
}

async fn all_lists(
    _: ListsPath,
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
        let mut stmt = db.connection.prepare("SELECT count(*) FROM list;")?;
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
    let res: Vec<_> = lists_values
        .into_iter()
        .skip(offset)
        .take(count)
        .map(DbVal::into_inner)
        .collect();

    Ok(Json(GetResponse {
        total: res.len(),
        start: offset,
        entries: res,
    }))
}

async fn new_list(
    _: ListsPath,
    State(_state): State<Arc<Configuration>>,
    //Json(_body): Json<GetRequest>,
) -> Result<Json<()>, ResponseError> {
    // TODO create new list
    Err(mailpot_web::ResponseError::new(
        "Not allowed".to_string(),
        StatusCode::UNAUTHORIZED,
    ))
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
    entries: Vec<MailingList>,
    total: usize,
    start: usize,
}

async fn get_list_owner(
    ListOwnerPath(id): ListOwnerPath,
    State(state): State<Arc<Configuration>>,
) -> Result<Json<Vec<ListOwner>>, ResponseError> {
    let db = Connection::open_db(Configuration::clone(&state))?;
    let owners = match id {
        ListPathIdentifier::Pk(id) => db.list_owners(id)?,
        ListPathIdentifier::Id(id) => {
            if let Some(owners) = db.list_by_id(id)?.map(|l| db.list_owners(l.pk())) {
                owners?
            } else {
                return Err(mailpot_web::ResponseError::new(
                    "Not found".to_string(),
                    StatusCode::NOT_FOUND,
                ));
            }
        }
    };
    Ok(Json(owners.into_iter().map(DbVal::into_inner).collect()))
}

async fn new_list_owner(
    ListOwnerPath(_id): ListOwnerPath,
    State(_state): State<Arc<Configuration>>,
    //Json(_body): Json<GetRequest>,
) -> Result<Json<Vec<ListOwner>>, ResponseError> {
    Err(mailpot_web::ResponseError::new(
        "Not allowed".to_string(),
        StatusCode::UNAUTHORIZED,
    ))
}

async fn get_list_subs(
    ListSubscriptionPath(id): ListSubscriptionPath,
    State(state): State<Arc<Configuration>>,
) -> Result<Json<Vec<ListSubscription>>, ResponseError> {
    let db = Connection::open_db(Configuration::clone(&state))?;
    let subs = match id {
        ListPathIdentifier::Pk(id) => db.list_subscriptions(id)?,
        ListPathIdentifier::Id(id) => {
            if let Some(v) = db.list_by_id(id)?.map(|l| db.list_subscriptions(l.pk())) {
                v?
            } else {
                return Err(mailpot_web::ResponseError::new(
                    "Not found".to_string(),
                    StatusCode::NOT_FOUND,
                ));
            }
        }
    };
    Ok(Json(subs.into_iter().map(DbVal::into_inner).collect()))
}

async fn new_list_sub(
    ListSubscriptionPath(_id): ListSubscriptionPath,
    State(_state): State<Arc<Configuration>>,
    //Json(_body): Json<GetRequest>,
) -> Result<Json<ListSubscription>, ResponseError> {
    Err(mailpot_web::ResponseError::new(
        "Not allowed".to_string(),
        StatusCode::UNAUTHORIZED,
    ))
}

#[cfg(test)]
mod tests {

    use axum::{
        body::Body,
        http::{method::Method, Request, StatusCode},
    };
    use mailpot::{models::*, Configuration, Connection, SendMail};
    use mailpot_tests::init_stderr_logging;
    use serde_json::json;
    use tempfile::TempDir;
    use tower::ServiceExt; // for `oneshot` and `ready`

    use super::*;

    #[tokio::test]
    async fn test_list_router() {
        init_stderr_logging();

        let tmp_dir = TempDir::new().unwrap();

        let db_path = tmp_dir.path().join("mpot.db");
        std::fs::copy("../mailpot-tests/for_testing.db", &db_path).unwrap();
        let mut perms = std::fs::metadata(&db_path).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        std::fs::set_permissions(&db_path, perms).unwrap();
        let config = Configuration {
            send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
            db_path,
            data_path: tmp_dir.path().to_path_buf(),
            administrators: vec![],
        };

        let db = Connection::open_db(config.clone()).unwrap().trusted();
        assert!(!db.lists().unwrap().is_empty());
        let foo_chat = MailingList {
            pk: 1,
            name: "foobar chat".into(),
            id: "foo-chat".into(),
            address: "foo-chat@example.com".into(),
            topics: vec![],
            description: None,
            archive_url: None,
        };
        assert_eq!(&db.lists().unwrap().remove(0).into_inner(), &foo_chat);
        drop(db);

        let config = Arc::new(config);

        // ------------------------------------------------------------
        // all_lists() get total

        let response = crate::create_app(config.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/list/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let r: GetResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(&r.entries, &[]);
        assert_eq!(r.total, 1);
        assert_eq!(r.start, 0);

        // ------------------------------------------------------------
        // all_lists() with count

        let response = crate::create_app(config.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/list/?count=20")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let r: GetResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(&r.entries, std::slice::from_ref(&foo_chat));
        assert_eq!(r.total, 1);
        assert_eq!(r.start, 0);

        // ------------------------------------------------------------
        // new_list()

        let response = crate::create_app(config.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/list/")
                    .header("Content-Type", "application/json")
                    .method(Method::POST)
                    .body(Body::from(serde_json::to_vec(&json! {{}}).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // ------------------------------------------------------------
        // get_list()

        let response = crate::create_app(config.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/list/1/")
                    .header("Content-Type", "application/json")
                    .method(Method::GET)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let r: MailingList = serde_json::from_slice(&body).unwrap();
        assert_eq!(&r, &foo_chat);

        // ------------------------------------------------------------
        // get_list_subs()

        let response = crate::create_app(config.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/list/1/subscription/")
                    .header("Content-Type", "application/json")
                    .method(Method::GET)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let r: Vec<ListSubscription> = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            &r,
            &[ListSubscription {
                pk: 1,
                list: 1,
                address: "user@example.com".to_string(),
                name: Some("Name".to_string()),
                account: Some(1),
                enabled: true,
                verified: false,
                digest: false,
                hide_address: false,
                receive_duplicates: true,
                receive_own_posts: false,
                receive_confirmation: true
            }]
        );

        // ------------------------------------------------------------
        // new_list_sub()

        let response = crate::create_app(config.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/list/1/subscription/")
                    .header("Content-Type", "application/json")
                    .method(Method::POST)
                    .body(Body::from(serde_json::to_vec(&json! {{}}).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // ------------------------------------------------------------
        // get_list_owner()

        let response = crate::create_app(config.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/list/1/owner/")
                    .header("Content-Type", "application/json")
                    .method(Method::GET)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let r: Vec<ListOwner> = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            &r,
            &[ListOwner {
                pk: 1,
                list: 1,
                address: "user@example.com".into(),
                name: None
            }]
        );

        // ------------------------------------------------------------
        // new_list_owner()

        let response = crate::create_app(config.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/list/1/owner/")
                    .header("Content-Type", "application/json")
                    .method(Method::POST)
                    .body(Body::from(serde_json::to_vec(&json! {{}}).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
