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

use std::{borrow::Cow, collections::HashMap, sync::Arc};

use axum::{extract::State, handler::Handler, response::Html, routing::get, Router};
use axum_login::AuthLayer;
use axum_sessions::{async_session::MemoryStore, extractors::WritableSession, SessionLayer};
use chrono::TimeZone;
use mailpot::{log, Configuration, Connection};
use mailpot_web::{
    auth::{logout_handler, Role},
    help::help,
    lists::{
        list, list_candidates, list_edit, list_edit_POST, list_post, list_post_eml, list_post_mbox,
        list_post_raw, list_subscribers,
    },
    minijinja_utils::{MailingList, TEMPLATES},
    settings::{settings, settings_POST, user_list_subscription, user_list_subscription_POST},
    topics::list_topics,
    typed_paths::{tsr::RouterExt, IntoCrumb, LoginPath},
    utils::{Crumb, SessionMessages},
    *,
};
use minijinja::value::Value;
use rand::Rng;
use tokio::sync::RwLock;

fn new_state(conf: Configuration) -> Arc<AppState> {
    Arc::new(AppState {
        conf,
        root_url_prefix: Value::from_safe_string(
            std::env::var("ROOT_URL_PREFIX").unwrap_or_default(),
        ),
        public_url: std::env::var("PUBLIC_URL").unwrap_or_else(|_| "localhost".to_string()),
        site_title: std::env::var("SITE_TITLE")
            .map(Cow::Owned)
            .unwrap_or_else(|_| Cow::Borrowed("mailing list archive")),
        site_subtitle: std::env::var("SITE_SUBTITLE").ok().map(Into::into),
        user_store: Arc::new(RwLock::new(HashMap::default())),
        ssh_namespace: std::env::var("SSH_NAMESPACE")
            .map(Cow::Owned)
            .unwrap_or_else(|_| Cow::Borrowed("mailpot")),
    })
}

fn create_app(shared_state: Arc<AppState>) -> Router {
    let store = MemoryStore::new();
    let secret = rand::thread_rng().gen::<[u8; 128]>();
    let session_layer = SessionLayer::new(store, &secret).with_secure(false);

    let auth_layer = AuthLayer::new(shared_state.clone(), &secret);

    let login_url =
        Arc::new(format!("{}{}", shared_state.root_url_prefix, LoginPath.to_crumb()).into());
    Router::new()
        .route("/", get(root))
        .typed_get(list)
        .typed_get(list_post)
        .typed_get(list_post_raw)
        .typed_get(list_topics)
        .typed_get(list_post_eml)
        .typed_get(list_post_mbox)
        .typed_get(list_edit.layer(RequireAuth::login_with_role_or_redirect(
            Role::User..,
            Arc::clone(&login_url),
            Some(Arc::new("next".into())),
        )))
        .typed_post(
            {
                let shared_state = Arc::clone(&shared_state);
                move |path, session, user, payload| {
                    list_edit_POST(path, session, user, payload, State(shared_state))
                }
            }
            .layer(RequireAuth::login_with_role_or_redirect(
                Role::User..,
                Arc::clone(&login_url),
                Some(Arc::new("next".into())),
            )),
        )
        .typed_get(
            list_subscribers.layer(RequireAuth::login_with_role_or_redirect(
                Role::User..,
                Arc::clone(&login_url),
                Some(Arc::new("next".into())),
            )),
        )
        .typed_get(
            list_candidates.layer(RequireAuth::login_with_role_or_redirect(
                Role::User..,
                Arc::clone(&login_url),
                Some(Arc::new("next".into())),
            )),
        )
        .typed_get(help)
        .typed_get(auth::ssh_signin)
        .typed_post({
            let shared_state = Arc::clone(&shared_state);
            move |path, session, query, auth, body| {
                auth::ssh_signin_POST(path, session, query, auth, body, shared_state)
            }
        })
        .typed_get(logout_handler)
        .typed_post(logout_handler)
        .typed_get(
            {
                let shared_state = Arc::clone(&shared_state);
                move |path, session, user| settings(path, session, user, shared_state)
            }
            .layer(RequireAuth::login_or_redirect(
                Arc::clone(&login_url),
                Some(Arc::new("next".into())),
            )),
        )
        .typed_post(
            {
                let shared_state = Arc::clone(&shared_state);
                move |path, session, auth, body| {
                    settings_POST(path, session, auth, body, shared_state)
                }
            }
            .layer(RequireAuth::login_or_redirect(
                Arc::clone(&login_url),
                Some(Arc::new("next".into())),
            )),
        )
        .typed_get(
            user_list_subscription.layer(RequireAuth::login_with_role_or_redirect(
                Role::User..,
                Arc::clone(&login_url),
                Some(Arc::new("next".into())),
            )),
        )
        .typed_post(
            {
                let shared_state = Arc::clone(&shared_state);
                move |session, path, user, body| {
                    user_list_subscription_POST(session, path, user, body, shared_state)
                }
            }
            .layer(RequireAuth::login_with_role_or_redirect(
                Role::User..,
                Arc::clone(&login_url),
                Some(Arc::new("next".into())),
            )),
        )
        .layer(auth_layer)
        .layer(session_layer)
        .with_state(shared_state)
}

#[tokio::main]
async fn main() {
    let config_path = std::env::args()
        .nth(1)
        .expect("Expected configuration file path as first argument.");
    if ["-v", "--version", "info"].contains(&config_path.as_str()) {
        println!("{}", crate::get_git_sha());
        println!("{CLI_INFO}");

        return;
    }
    #[cfg(test)]
    let verbosity = log::LevelFilter::Trace;
    #[cfg(not(test))]
    let verbosity = log::LevelFilter::Info;
    stderrlog::new()
        .quiet(false)
        .verbosity(verbosity)
        .show_module_names(true)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();
    let conf = Configuration::from_file(config_path).unwrap();
    let app = create_app(new_state(conf));

    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listen_to = format!("{hostname}:{port}");
    println!("Listening to {listen_to}...");
    axum::Server::bind(&listen_to.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn root(
    mut session: WritableSession,
    auth: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, ResponseError> {
    let db = Connection::open_db(state.conf.clone())?;
    let lists_values = db.lists()?;
    let lists = lists_values
        .iter()
        .map(|list| {
            let months = db.months(list.pk)?;
            let posts = db.list_posts(list.pk, None)?;
            let newest = posts.last().and_then(|p| {
                chrono::Utc
                    .timestamp_opt(p.timestamp as i64, 0)
                    .earliest()
                    .map(|d| d.to_rfc3339())
            });
            let list_owners = db.list_owners(list.pk)?;
            let mut list_obj = MailingList::from(list.clone());
            list_obj.set_safety(list_owners.as_slice(), &state.conf.administrators);
            Ok(minijinja::context! {
                newest,
                posts => &posts,
                months => &months,
                list => Value::from_object(list_obj),
            })
        })
        .collect::<Result<Vec<_>, mailpot::Error>>()?;
    let crumbs = vec![Crumb {
        label: "Home".into(),
        url: "/".into(),
    }];

    let context = minijinja::context! {
        page_title => Option::<&'static str>::None,
        lists => &lists,
        current_user => auth.current_user,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Ok(Html(TEMPLATES.get_template("lists.html")?.render(context)?))
}

#[cfg(test)]
mod tests {

    use axum::{
        body::Body,
        http::{
            header::{COOKIE, SET_COOKIE},
            method::Method,
            Request, Response, StatusCode,
        },
    };
    use axum_extra::routing::TypedPath;
    use mailpot::{Configuration, Connection, SendMail};
    use mailpot_tests::init_stderr_logging;
    use mailpot_web::{
        auth::{AuthFormPayload, User},
        typed_paths::*,
        utils::*,
    };
    use percent_encoding::utf8_percent_encode;
    use tempfile::TempDir;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn test_routes() {
        init_stderr_logging();

        macro_rules! req {
            (get $url:expr) => {{
                Request::builder()
                    .uri($url)
                    .method(Method::GET)
                    .body(Body::empty())
                    .unwrap()
            }};
            (post $url:expr, $body:expr) => {{
                Request::builder()
                    .uri($url)
                    .method(Method::POST)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(Body::from(
                        serde_urlencoded::to_string($body).unwrap().into_bytes(),
                    ))
                    .unwrap()
            }};
        }

        let tmp_dir = TempDir::new().unwrap();

        let db_path = tmp_dir.path().join("mpot.db");
        {
            let conn = mailpot::rusqlite::Connection::open(&db_path).unwrap();
            conn.execute_batch(include_str!("../../mailpot-tests/for_testing.sql"))
                .unwrap();
            conn.close().unwrap();
        }
        let config = Configuration {
            send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
            db_path,
            data_path: tmp_dir.path().to_path_buf(),
            administrators: vec![],
        };
        let db = Connection::open_db(config.clone()).unwrap();
        let list = db.lists().unwrap().remove(0);

        let state = new_state(config.clone());

        // ------------------------------------------------------------
        // list()

        let cl = |url, state| async move {
            let response = create_app(state).oneshot(req!(get & url)).await.unwrap();

            assert_eq!(response.status(), StatusCode::OK);

            hyper::body::to_bytes(response.into_body()).await.unwrap()
        };
        assert_eq!(
            cl(format!("/list/{}/", list.id), state.clone()).await,
            cl(format!("/list/{}/", list.pk), state.clone()).await
        );

        let vnu = |res: Response<_>| async move {
            use std::process::Stdio;

            use tokio::{io::AsyncWriteExt, process::Command};

            // Change value to run vnu validator tests
            // [ref:TODO]: do vnu in standalone test?
            const RUN_VNU: bool = false;

            let body = hyper::body::to_bytes(res.into_body()).await.unwrap();

            if !RUN_VNU {
                return;
            }

            let mut child = Command::new("vnu")
                .arg("--Werror")
                .arg("--also-check-css")
                .arg("--filterpattern")
                .arg(".*Possible misuse of .aria-label..*")
                .arg("-")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::piped())
                .spawn()
                .expect("failed to spawn command");

            let mut stdin = child
                .stdin
                .take()
                .expect("child did not have a handle to stdin");

            stdin
                .write_all(&body)
                .await
                .expect("could not write to stdin");

            drop(stdin);

            let op = child.wait_with_output().await.unwrap();

            if !op.status.success() {
                std::fs::write("/tmp/failure.html", &body).unwrap();
                panic!(
                    "vnu exited with {}:\nstdout: {}\n\nstderr: {}",
                    op.status.code().unwrap_or(-1),
                    String::from_utf8_lossy(&op.stdout),
                    String::from_utf8_lossy(&op.stderr)
                );
            }
        };

        // ------------------------------------------------------------
        // list_post(), list_post_eml(), list_post_raw()

        {
            let msg_id = "<abcdefgh@sator.example.com>";
            let res = create_app(state.clone())
                .oneshot(req!(
                    get & format!(
                        "/list/{id}/posts/{msgid}/",
                        id = list.id,
                        msgid = utf8_percent_encode(msg_id, mailpot::PATH_SEGMENT)
                    )
                ))
                .await
                .unwrap();

            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(
                res.headers().get(http::header::CONTENT_TYPE),
                Some(&http::HeaderValue::from_static("text/html; charset=utf-8"))
            );
            vnu(res).await;
            let res = create_app(state.clone())
                .oneshot(req!(
                    get & format!(
                        "/list/{id}/posts/{msgid}/raw/",
                        id = list.id,
                        msgid = utf8_percent_encode(msg_id, mailpot::PATH_SEGMENT)
                    )
                ))
                .await
                .unwrap();

            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(
                res.headers().get(http::header::CONTENT_TYPE),
                Some(&http::HeaderValue::from_static("text/plain; charset=utf-8"))
            );
            let res = create_app(state.clone())
                .oneshot(req!(
                    get & format!(
                        "/list/{id}/posts/{msgid}/eml/",
                        id = list.id,
                        msgid = utf8_percent_encode(msg_id, mailpot::PATH_SEGMENT)
                    )
                ))
                .await
                .unwrap();

            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(
                res.headers().get(http::header::CONTENT_TYPE),
                Some(&http::HeaderValue::from_static("application/octet-stream"))
            );
            assert_eq!(
                res.headers().get(http::header::CONTENT_DISPOSITION),
                Some(&http::HeaderValue::from_static(
                    "attachment; filename=\"abcdefgh@sator.example.com.eml\""
                )),
            );
        }
        // ------------------------------------------------------------
        // help(), ssh_signin(), root()

        for path in ["/help/", "/"] {
            let res = create_app(state.clone())
                .oneshot(req!(get path))
                .await
                .unwrap();

            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(
                res.headers().get(http::header::CONTENT_TYPE),
                Some(&http::HeaderValue::from_static("text/html; charset=utf-8"))
            );
            vnu(res).await;
        }

        if cfg!(not(debug_assertions)) {
            return;
        }
        // ------------------------------------------------------------
        // auth.rs...

        let login_app = create_app(state.clone());
        let session_cookie = {
            let res = login_app
                .clone()
                .oneshot(req!(get "/login/"))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(
                res.headers().get(http::header::CONTENT_TYPE),
                Some(&http::HeaderValue::from_static("text/html; charset=utf-8"))
            );

            let cookie = res.headers().get(SET_COOKIE).unwrap().clone();
            vnu(res).await;
            cookie
        };
        let user = User {
            pk: 1,
            ssh_signature: String::new(),
            role: Role::User,
            public_key: None,
            password: String::new(),
            name: None,
            address: String::new(),
            enabled: true,
        };
        state.insert_user(1, user.clone()).await;

        {
            let mut request = req!(post "/login/",
                AuthFormPayload {
                    address: "user@example.com".into(),
                    password: "hunter2".into()
                }
            );
            request
                .headers_mut()
                .insert(COOKIE, session_cookie.to_owned());
            let res = login_app.clone().oneshot(request).await.unwrap();

            assert_eq!(
                res.headers().get(http::header::LOCATION),
                Some(
                    &SettingsPath
                        .to_uri()
                        .to_string()
                        .as_str()
                        .try_into()
                        .unwrap()
                )
            );
        }

        // ------------------------------------------------------------
        // settings()

        {
            let mut request = req!(get "/settings/");
            request
                .headers_mut()
                .insert(COOKIE, session_cookie.to_owned());
            let res = login_app.clone().oneshot(request).await.unwrap();

            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(
                res.headers().get(http::header::CONTENT_TYPE),
                Some(&http::HeaderValue::from_static("text/html; charset=utf-8"))
            );
            vnu(res).await;
        }

        // ------------------------------------------------------------
        // settings_post()

        {
            let mut request = req!(
            post "/settings/",
            crate::settings::ChangeSetting::Subscribe {
                list_pk: IntPOST(1),
            });
            request
                .headers_mut()
                .insert(COOKIE, session_cookie.to_owned());
            let res = login_app.clone().oneshot(request).await.unwrap();

            assert_eq!(
                res.headers().get(http::header::LOCATION),
                Some(
                    &SettingsPath
                        .to_uri()
                        .to_string()
                        .as_str()
                        .try_into()
                        .unwrap()
                )
            );
        }
        // ------------------------------------------------------------
        // user_list_subscription() TODO

        // ------------------------------------------------------------
        // user_list_subscription_post() TODO

        // ------------------------------------------------------------
        // list_edit()

        {
            let mut request = req!(get & format!("/list/{id}/edit/", id = list.id,));
            request
                .headers_mut()
                .insert(COOKIE, session_cookie.to_owned());
            let res = login_app.clone().oneshot(request).await.unwrap();

            assert_eq!(res.status(), StatusCode::OK);
            assert_eq!(
                res.headers().get(http::header::CONTENT_TYPE),
                Some(&http::HeaderValue::from_static("text/html; charset=utf-8"))
            );
            vnu(res).await;
        }

        // ------------------------------------------------------------
        // list_edit_POST()

        {
            let mut request = req!(
                post & format!("/list/{id}/edit/", id = list.id,),
                crate::lists::ChangeSetting::Metadata {
                    name: "new name".to_string(),
                    id: "new-name".to_string(),
                    address: list.address.clone(),
                    description: list.description.clone(),
                    owner_local_part: None,
                    request_local_part: None,
                    archive_url: None,
                }
            );
            request
                .headers_mut()
                .insert(COOKIE, session_cookie.to_owned());
            let response = login_app.clone().oneshot(request).await.unwrap();

            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            let list_mod = db.lists().unwrap().remove(0);
            assert_eq!(&list_mod.name, "new name");
            assert_eq!(&list_mod.id, "new-name");
            assert_eq!(&list_mod.address, &list.address);
            assert_eq!(&list_mod.description, &list.description);
        }

        {
            let mut request = req!(post "/list/new-name/edit/",
                crate::lists::ChangeSetting::SubscriptionPolicy {
                    send_confirmation: BoolPOST(false),
                    subscription_policy: crate::lists::SubscriptionPolicySettings::Custom,
                }
            );
            request
                .headers_mut()
                .insert(COOKIE, session_cookie.to_owned());
            let response = login_app.clone().oneshot(request).await.unwrap();

            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            let policy = db.list_subscription_policy(list.pk()).unwrap().unwrap();
            assert!(!policy.send_confirmation);
            assert!(policy.custom);
        }
        {
            let mut request = req!(post "/list/new-name/edit/",
                crate::lists::ChangeSetting::PostPolicy {
                    delete_post_policy: None,
                    post_policy: crate::lists::PostPolicySettings::Custom,
                }
            );
            request
                .headers_mut()
                .insert(COOKIE, session_cookie.to_owned());
            let response = login_app.clone().oneshot(request).await.unwrap();

            assert_eq!(response.status(), StatusCode::SEE_OTHER);
            let policy = db.list_post_policy(list.pk()).unwrap().unwrap();
            assert!(policy.custom);
        }
    }
}
