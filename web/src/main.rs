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

use mpot_web::*;
use rand::Rng;

use minijinja::value::Value;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let config_path = std::env::args()
        .nth(1)
        .expect("Expected configuration file path as first argument.");
    let conf = Configuration::from_file(config_path).unwrap();

    let store = MemoryStore::new();
    let secret = rand::thread_rng().gen::<[u8; 128]>();
    let session_layer = SessionLayer::new(store, &secret).with_secure(false);

    let shared_state = Arc::new(AppState {
        conf,
        root_url_prefix: String::new(),
        public_url: "lists.mailpot.rs".into(),
        user_store: Arc::new(RwLock::new(HashMap::default())),
    });

    let auth_layer = AuthLayer::new(shared_state.clone(), &secret);

    let app = Router::new()
        .route("/", get(root))
        .route("/lists/:pk/", get(list))
        .route("/lists/:pk/edit/", get(list_edit))
        .route("/help/", get(help))
        .route(
            "/login/",
            get(auth::ssh_signin).post({
                let shared_state = Arc::clone(&shared_state);
                move |session, auth, body| auth::ssh_signin_post(session, auth, body, shared_state)
            }),
        )
        .route("/logout/", get(logout_handler))
        .route(
            "/settings/",
            get({
                let shared_state = Arc::clone(&shared_state);
                move |session, user| settings(session, user, shared_state)
            }
            .layer(RequireAuth::login()))
            .post(
                {
                    let shared_state = Arc::clone(&shared_state);
                    move |session, auth, body| settings_post(session, auth, body, shared_state)
                }
                .layer(RequireAuth::login()),
            ),
        )
        .route(
            "/settings/list/:pk/",
            get(user_list_subscription)
                .layer(RequireAuth::login_with_role(Role::User..))
                .post({
                    let shared_state = Arc::clone(&shared_state);
                    move |session, path, user, body| {
                        user_list_subscription_post(session, path, user, body, shared_state)
                    }
                })
                .layer(RequireAuth::login_with_role(Role::User..)),
        )
        .layer(auth_layer)
        .layer(session_layer)
        .with_state(shared_state);

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
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
        label: "Lists".into(),
        url: "/".into(),
    }];

    let context = minijinja::context! {
        title => "mailing list archive",
        description => "",
        lists => &lists,
        root_url_prefix => &state.root_url_prefix,
        current_user => auth.current_user,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Ok(Html(TEMPLATES.get_template("lists.html")?.render(context)?))
}

async fn list(
    mut session: WritableSession,
    Path(id): Path<i64>,
    auth: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, ResponseError> {
    let db = Connection::open_db(state.conf.clone())?;
    let list = db.list(id)?;
    let post_policy = db.list_policy(list.pk)?;
    let subscription_policy = db.list_subscription_policy(list.pk)?;
    let months = db.months(list.pk)?;
    let user_context = auth
        .current_user
        .as_ref()
        .map(|user| db.list_subscription_by_address(id, &user.address).ok());

    let posts = db.list_posts(list.pk, None)?;
    let mut hist = months
        .iter()
        .map(|m| (m.to_string(), [0usize; 31]))
        .collect::<HashMap<String, [usize; 31]>>();
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
                address => post.address,
                message_id => msg_id,
                message => post.message,
                timestamp => post.timestamp,
                datetime => post.datetime,
                root_url_prefix => &state.root_url_prefix,
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
        title => &list.name,
        description => &list.description,
        post_policy => &post_policy,
        subscription_policy => &subscription_policy,
        preamble => true,
        months => &months,
        hists => &hist,
        posts => posts_ctx,
        body => &list.description.clone().unwrap_or_default(),
        root_url_prefix => &state.root_url_prefix,
        list => Value::from_object(MailingList::from(list)),
        current_user => auth.current_user,
        user_context => user_context,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Ok(Html(TEMPLATES.get_template("list.html")?.render(context)?))
}

async fn list_edit(Path(_): Path<i64>, State(_): State<Arc<AppState>>) {}

async fn help(
    mut session: WritableSession,
    auth: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, ResponseError> {
    let crumbs = vec![
        Crumb {
            label: "Lists".into(),
            url: "/".into(),
        },
        Crumb {
            label: "Help".into(),
            url: "/help/".into(),
        },
    ];
    let context = minijinja::context! {
        title => "Help & Documentation",
        description => "",
        root_url_prefix => &state.root_url_prefix,
        current_user => auth.current_user,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Ok(Html(TEMPLATES.get_template("help.html")?.render(context)?))
}
