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
        .typed_get(list_edit)
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
    axum::Server::bind(&format!("{hostname}:{port}").parse().unwrap())
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

async fn list(
    ListPath(id): ListPath,
    mut session: WritableSession,
    auth: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, ResponseError> {
    let db = Connection::open_db(state.conf.clone())?;
    let Some(list) = (match id {
        ListPathIdentifier::Pk(id) => db.list(id)?,
        ListPathIdentifier::Id(id) => db.list_by_id(id)?,
    }) else {
        return Err(ResponseError::new(
            "List not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };
    let post_policy = db.list_post_policy(list.pk)?;
    let subscription_policy = db.list_subscription_policy(list.pk)?;
    let months = db.months(list.pk)?;
    let user_context = auth
        .current_user
        .as_ref()
        .map(|user| db.list_subscription_by_address(list.pk, &user.address).ok());

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
            label: "Home".into(),
            url: "/".into(),
        },
        Crumb {
            label: list.name.clone().into(),
            url: ListPath(list.pk().into()).to_crumb(),
        },
    ];
    let context = minijinja::context! {
        title => state.site_title.as_ref(),
        page_title => &list.name,
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

async fn list_post(
    ListPostPath(id, msg_id): ListPostPath,
    mut session: WritableSession,
    auth: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, ResponseError> {
    let db = Connection::open_db(state.conf.clone())?;
    let Some(list) = (match id {
        ListPathIdentifier::Pk(id) => db.list(id)?,
        ListPathIdentifier::Id(id) => db.list_by_id(id)?,
    }) else {
        return Err(ResponseError::new(
            "List not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };
    let user_context = auth.current_user.as_ref().map(|user| {
        db.list_subscription_by_address(list.pk(), &user.address)
            .ok()
    });

    let post = if let Some(post) = db.list_post_by_message_id(list.pk, &msg_id)? {
        post
    } else {
        return Err(ResponseError::new(
            format!("Post with Message-ID {} not found", msg_id),
            StatusCode::NOT_FOUND,
        ));
    };
    let envelope = melib::Envelope::from_bytes(post.message.as_slice(), None)
        .with_status(StatusCode::BAD_REQUEST)?;
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
    let crumbs = vec![
        Crumb {
            label: "Home".into(),
            url: "/".into(),
        },
        Crumb {
            label: list.name.clone().into(),
            url: ListPath(list.pk().into()).to_crumb(),
        },
        Crumb {
            label: format!("{} {msg_id}", subject_ref).into(),
            url: ListPostPath(list.pk().into(), msg_id.to_string()).to_crumb(),
        },
    ];
    let context = minijinja::context! {
        title => state.site_title.as_ref(),
        page_title => subject_ref,
        description => &list.description,
        list => Value::from_object(MailingList::from(list)),
        pk => post.pk,
        body => &body_text,
        from => &envelope.field_from_to_string(),
        date => &envelope.date_as_str(),
        to => &envelope.field_to_to_string(),
        subject => &envelope.subject(),
        trimmed_subject => subject_ref,
        in_reply_to => &envelope.in_reply_to_display().map(|r| r.to_string().as_str().strip_carets().to_string()),
        references => &envelope.references().into_iter().map(|m| m.to_string().as_str().strip_carets().to_string()).collect::<Vec<String>>(),
        message_id => msg_id,
        message => post.message,
        timestamp => post.timestamp,
        datetime => post.datetime,
        root_url_prefix => &state.root_url_prefix,
        current_user => auth.current_user,
        user_context => user_context,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Ok(Html(TEMPLATES.get_template("post.html")?.render(context)?))
}

async fn list_edit(ListEditPath(_): ListEditPath, State(_): State<Arc<AppState>>) {}

async fn help(
    _: HelpPath,
    mut session: WritableSession,
    auth: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, ResponseError> {
    let crumbs = vec![
        Crumb {
            label: "Home".into(),
            url: "/".into(),
        },
        Crumb {
            label: "Help".into(),
            url: HelpPath.to_crumb(),
        },
    ];
    let context = minijinja::context! {
        title => state.site_title.as_ref(),
        page_title => "Help & Documentation",
        description => "",
        root_url_prefix => &state.root_url_prefix,
        current_user => auth.current_user,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Ok(Html(TEMPLATES.get_template("help.html")?.render(context)?))
}
