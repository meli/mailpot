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

use std::{collections::HashMap, sync::Arc};

use mailpot_web::*;
use minijinja::value::Value;
use rand::Rng;
use tokio::sync::RwLock;

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
    let conf = Configuration::from_file(config_path).unwrap();

    let store = MemoryStore::new();
    let secret = rand::thread_rng().gen::<[u8; 128]>();
    let session_layer = SessionLayer::new(store, &secret).with_secure(false);

    let shared_state = Arc::new(AppState {
        conf,
        root_url_prefix: Value::from_safe_string(
            std::env::var("ROOT_URL_PREFIX").unwrap_or_default(),
        ),
        public_url: std::env::var("PUBLIC_URL").unwrap_or_else(|_| "lists.mailpot.rs".to_string()),
        site_title: std::env::var("SITE_TITLE")
            .unwrap_or_else(|_| "mailing list archive".to_string())
            .into(),
        user_store: Arc::new(RwLock::new(HashMap::default())),
    });

    let auth_layer = AuthLayer::new(shared_state.clone(), &secret);

    let login_url =
        Arc::new(format!("{}{}", shared_state.root_url_prefix, LoginPath.to_crumb()).into());
    let app = Router::new()
        .route("/", get(root))
        .typed_get(list)
        .typed_get(list_post)
        .typed_get(list_edit.layer(RequireAuth::login_with_role_or_redirect(
            Role::User..,
            Arc::clone(&login_url),
            Some(Arc::new("next".into())),
        )))
        .typed_post(
            {
                let shared_state = Arc::clone(&shared_state);
                move |path, session, user, payload| {
                    list_edit_post(path, session, user, payload, State(shared_state))
                }
            }
            .layer(RequireAuth::login_with_role_or_redirect(
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
                auth::ssh_signin_post(path, session, query, auth, body, shared_state)
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
                    settings_post(path, session, auth, body, shared_state)
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
                    user_list_subscription_post(session, path, user, body, shared_state)
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
        .with_state(shared_state);

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
            Ok(minijinja::context! {
                name => &list.name,
                posts => &posts,
                months => &months,
                body => &list.description.as_deref().unwrap_or_default(),
                root_url_prefix => &state.root_url_prefix,
                list => Value::from_object(MailingList::from(list.clone())),
            })
        })
        .collect::<Result<Vec<_>, mailpot::Error>>()?;
    let crumbs = vec![Crumb {
        label: "Home".into(),
        url: "/".into(),
    }];

    let context = minijinja::context! {
        title => state.site_title.as_ref(),
        page_title => Option::<&'static str>::None,
        description => "",
        lists => &lists,
        root_url_prefix => &state.root_url_prefix,
        current_user => auth.current_user,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Ok(Html(TEMPLATES.get_template("lists.html")?.render(context)?))
}
