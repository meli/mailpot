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

use chrono::TimeZone;
use indexmap::IndexMap;
use mailpot::{models::Post, StripCarets, StripCaretsInplace};

use super::*;

/// Mailing list index.
pub async fn list(
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
    let post_map = posts
        .iter()
        .map(|p| (p.message_id.as_str(), p))
        .collect::<IndexMap<&str, &mailpot::models::DbVal<mailpot::models::Post>>>();
    let mut hist = months
        .iter()
        .map(|m| (m.to_string(), [0usize; 31]))
        .collect::<HashMap<String, [usize; 31]>>();
    let envelopes: Arc<std::sync::RwLock<HashMap<melib::EnvelopeHash, melib::Envelope>>> =
        Default::default();
    {
        let mut env_lock = envelopes.write().unwrap();

        for post in &posts {
            let Ok(mut envelope) = melib::Envelope::from_bytes(post.message.as_slice(), None)
            else {
                continue;
            };
            if envelope.message_id != post.message_id.as_str() {
                // If they don't match, the raw envelope doesn't contain a Message-ID and it was
                // randomly generated. So set the envelope's Message-ID to match the
                // post's, which is the "permanent" one since our source of truth is
                // the database.
                envelope.set_message_id(post.message_id.as_bytes());
            }
            env_lock.insert(envelope.hash(), envelope);
        }
    }
    let mut threads: melib::Threads = melib::Threads::new(posts.len());
    threads.amend(&envelopes);
    let roots = thread_roots(&envelopes, &threads);
    let posts_ctx = roots
        .into_iter()
        .filter_map(|(thread, length, _timestamp)| {
            let post = &post_map[&thread.message_id.as_str()];
            //2019-07-14T14:21:02
            if let Some(day) =
                chrono::DateTime::<chrono::FixedOffset>::parse_from_rfc2822(post.datetime.trim())
                    .ok()
                    .map(|d| d.day())
            {
                hist.get_mut(&post.month_year).unwrap()[day.saturating_sub(1) as usize] += 1;
            }
            let envelope = melib::Envelope::from_bytes(post.message.as_slice(), None).ok()?;
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
            let ret = minijinja::context! {
                pk => post.pk,
                list => post.list,
                subject => subject_ref,
                address => post.address,
                message_id => msg_id,
                message => post.message,
                timestamp => post.timestamp,
                datetime => post.datetime,
                replies => length.saturating_sub(1),
                last_active => thread.datetime,
            };
            Some(ret)
        })
        .collect::<Vec<_>>();
    let crumbs = vec![
        Crumb {
            label: "Home".into(),
            url: "/".into(),
        },
        Crumb {
            label: list.name.clone().into(),
            url: ListPath(list.id.to_string().into()).to_crumb(),
        },
    ];
    let list_owners = db.list_owners(list.pk)?;
    let mut list_obj = MailingList::from(list.clone());
    list_obj.set_safety(list_owners.as_slice(), &state.conf.administrators);
    let context = minijinja::context! {
        canonical_url => ListPath::from(&list).to_crumb(),
        page_title => &list.name,
        description => &list.description,
        post_policy,
        subscription_policy,
        preamble => true,
        months,
        hists => &hist,
        posts => posts_ctx,
        list => Value::from_object(list_obj),
        current_user => auth.current_user,
        user_context,
        messages => session.drain_messages(),
        crumbs,
    };
    Ok(Html(
        TEMPLATES.get_template("lists/list.html")?.render(context)?,
    ))
}

/// Mailing list post page.
pub async fn list_post(
    ListPostPath(id, msg_id): ListPostPath,
    mut session: WritableSession,
    auth: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, ResponseError> {
    let db = Connection::open_db(state.conf.clone())?.trusted();
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
    let thread: Vec<(i64, DbVal<Post>, String, String)> = {
        let thread: Vec<(i64, DbVal<Post>)> = db.list_thread(list.pk, &post.message_id)?;

        thread
            .into_iter()
            .map(|(depth, p)| {
                let envelope = melib::Envelope::from_bytes(p.message.as_slice(), None).unwrap();
                let body = envelope.body_bytes(p.message.as_slice());
                let body_text = body.text(melib::attachment_types::Text::Rfc822);
                let date = envelope.date_as_str().to_string();
                (depth, p, body_text, date)
            })
            .collect()
    };
    let envelope = melib::Envelope::from_bytes(post.message.as_slice(), None)
        .with_status(StatusCode::BAD_REQUEST)?;
    let body = envelope.body_bytes(post.message.as_slice());
    let body_text = body.text(melib::attachment_types::Text::Rfc822);
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
            url: ListPath(list.id.to_string().into()).to_crumb(),
        },
        Crumb {
            label: format!("{} <{}>", subject_ref, msg_id.as_str().strip_carets()).into(),
            url: ListPostPath(list.id.to_string().into(), msg_id.to_string()).to_crumb(),
        },
    ];

    let list_owners = db.list_owners(list.pk)?;
    let mut list_obj = MailingList::from(list.clone());
    list_obj.set_safety(list_owners.as_slice(), &state.conf.administrators);

    let context = minijinja::context! {
        canonical_url => ListPostPath(ListPathIdentifier::from(list.id.clone()), msg_id.to_string().strip_carets_inplace()).to_crumb(),
        page_title => subject_ref,
        description => &list.description,
        list => Value::from_object(list_obj),
        pk => post.pk,
        body => &body_text,
        from => &envelope.field_from_to_string(),
        date => &envelope.date_as_str(),
        to => &envelope.field_to_to_string(),
        subject => &envelope.subject(),
        trimmed_subject => subject_ref,
        in_reply_to => &envelope.in_reply_to_display().map(|r| r.to_string().strip_carets_inplace()),
        references => &envelope.references().into_iter().map(|m| m.to_string().strip_carets_inplace()).collect::<Vec<String>>(),
        message_id => msg_id,
        message => post.message,
        timestamp => post.timestamp,
        datetime => post.datetime,
        thread => thread,
        current_user => auth.current_user,
        user_context => user_context,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Ok(Html(
        TEMPLATES.get_template("lists/post.html")?.render(context)?,
    ))
}

pub async fn list_edit(
    ListEditPath(id): ListEditPath,
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
            "Not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };
    let list_owners = db.list_owners(list.pk)?;
    let user_address = &auth.current_user.as_ref().unwrap().address;
    if !list_owners.iter().any(|o| &o.address == user_address) {
        return Err(ResponseError::new(
            "Not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };

    let post_policy = db.list_post_policy(list.pk)?;
    let subscription_policy = db.list_subscription_policy(list.pk)?;
    let post_count = {
        let mut stmt = db
            .connection
            .prepare("SELECT count(*) FROM post WHERE list = ?;")?;
        stmt.query_row([&list.pk], |row| {
            let count: usize = row.get(0)?;
            Ok(count)
        })
        .optional()?
        .unwrap_or(0)
    };
    let subs_count = {
        let mut stmt = db
            .connection
            .prepare("SELECT count(*) FROM subscription WHERE list = ?;")?;
        stmt.query_row([&list.pk], |row| {
            let count: usize = row.get(0)?;
            Ok(count)
        })
        .optional()?
        .unwrap_or(0)
    };
    let sub_requests_count = {
        let mut stmt = db.connection.prepare(
            "SELECT count(*) FROM candidate_subscription WHERE list = ? AND accepted IS NULL;",
        )?;
        stmt.query_row([&list.pk], |row| {
            let count: usize = row.get(0)?;
            Ok(count)
        })
        .optional()?
        .unwrap_or(0)
    };

    let crumbs = vec![
        Crumb {
            label: "Home".into(),
            url: "/".into(),
        },
        Crumb {
            label: list.name.clone().into(),
            url: ListPath(list.id.to_string().into()).to_crumb(),
        },
        Crumb {
            label: format!("Edit {}", list.name).into(),
            url: ListEditPath(ListPathIdentifier::from(list.id.clone())).to_crumb(),
        },
    ];
    let list_owners = db.list_owners(list.pk)?;
    let mut list_obj = MailingList::from(list.clone());
    list_obj.set_safety(list_owners.as_slice(), &state.conf.administrators);
    let context = minijinja::context! {
        canonical_url => ListEditPath(ListPathIdentifier::from(list.id.clone())).to_crumb(),
        page_title => format!("Edit {} settings", list.name),
        description => &list.description,
        post_policy,
        subscription_policy,
        list_owners,
        post_count,
        subs_count,
        sub_requests_count,
        list => Value::from_object(list_obj),
        current_user => auth.current_user,
        messages => session.drain_messages(),
        crumbs,
    };
    Ok(Html(
        TEMPLATES.get_template("lists/edit.html")?.render(context)?,
    ))
}

#[allow(non_snake_case)]
pub async fn list_edit_POST(
    ListEditPath(id): ListEditPath,
    mut session: WritableSession,
    Extension(user): Extension<User>,
    Form(payload): Form<ChangeSetting>,
    State(state): State<Arc<AppState>>,
) -> Result<Redirect, ResponseError> {
    let db = Connection::open_db(state.conf.clone())?;
    let Some(list) = (match id {
        ListPathIdentifier::Pk(id) => db.list(id)?,
        ListPathIdentifier::Id(ref id) => db.list_by_id(id)?,
    }) else {
        return Err(ResponseError::new(
            "Not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };
    let list_owners = db.list_owners(list.pk)?;
    let user_address = &user.address;
    if !list_owners.iter().any(|o| &o.address == user_address) {
        return Err(ResponseError::new(
            "Not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };

    let db = db.trusted();
    match payload {
        ChangeSetting::PostPolicy {
            delete_post_policy: _,
            post_policy: val,
        } => {
            use PostPolicySettings::*;
            session.add_message(
                if let Err(err) = db.set_list_post_policy(mailpot::models::PostPolicy {
                    pk: -1,
                    list: list.pk,
                    announce_only: matches!(val, AnnounceOnly),
                    subscription_only: matches!(val, SubscriptionOnly),
                    approval_needed: matches!(val, ApprovalNeeded),
                    open: matches!(val, Open),
                    custom: matches!(val, Custom),
                }) {
                    Message {
                        message: err.to_string().into(),
                        level: Level::Error,
                    }
                } else {
                    Message {
                        message: "Post policy saved.".into(),
                        level: Level::Success,
                    }
                },
            )?;
        }
        ChangeSetting::SubscriptionPolicy {
            send_confirmation: BoolPOST(send_confirmation),
            subscription_policy: val,
        } => {
            use SubscriptionPolicySettings::*;
            session.add_message(
                if let Err(err) =
                    db.set_list_subscription_policy(mailpot::models::SubscriptionPolicy {
                        pk: -1,
                        list: list.pk,
                        send_confirmation,
                        open: matches!(val, Open),
                        manual: matches!(val, Manual),
                        request: matches!(val, Request),
                        custom: matches!(val, Custom),
                    })
                {
                    Message {
                        message: err.to_string().into(),
                        level: Level::Error,
                    }
                } else {
                    Message {
                        message: "Subscription policy saved.".into(),
                        level: Level::Success,
                    }
                },
            )?;
        }
        ChangeSetting::Metadata {
            name,
            id,
            address,
            description,
            owner_local_part,
            request_local_part,
            archive_url,
        } => {
            session.add_message(
                if let Err(err) =
                    db.update_list(mailpot::models::changesets::MailingListChangeset {
                        pk: list.pk,
                        name: Some(name),
                        id: Some(id),
                        address: Some(address),
                        description: description.map(|s| if s.is_empty() { None } else { Some(s) }),
                        owner_local_part: owner_local_part.map(|s| {
                            if s.is_empty() {
                                None
                            } else {
                                Some(s)
                            }
                        }),
                        request_local_part: request_local_part.map(|s| {
                            if s.is_empty() {
                                None
                            } else {
                                Some(s)
                            }
                        }),
                        archive_url: archive_url.map(|s| if s.is_empty() { None } else { Some(s) }),
                        ..Default::default()
                    })
                {
                    Message {
                        message: err.to_string().into(),
                        level: Level::Error,
                    }
                } else {
                    Message {
                        message: "List metadata saved.".into(),
                        level: Level::Success,
                    }
                },
            )?;
        }
        ChangeSetting::AcceptSubscriptionRequest { pk: IntPOST(pk) } => {
            session.add_message(match db.accept_candidate_subscription(pk) {
                Ok(subscription) => Message {
                    message: format!("Added: {subscription:#?}").into(),
                    level: Level::Success,
                },
                Err(err) => Message {
                    message: format!("Could not accept subscription request! Reason: {err}").into(),
                    level: Level::Error,
                },
            })?;
        }
    }

    Ok(Redirect::to(&format!(
        "{}{}",
        &state.root_url_prefix,
        ListEditPath(id).to_uri()
    )))
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ChangeSetting {
    PostPolicy {
        #[serde(rename = "delete-post-policy", default)]
        delete_post_policy: Option<String>,
        #[serde(rename = "post-policy")]
        post_policy: PostPolicySettings,
    },
    SubscriptionPolicy {
        #[serde(rename = "send-confirmation", default)]
        send_confirmation: BoolPOST,
        #[serde(rename = "subscription-policy")]
        subscription_policy: SubscriptionPolicySettings,
    },
    Metadata {
        name: String,
        id: String,
        #[serde(default)]
        address: String,
        #[serde(default)]
        description: Option<String>,
        #[serde(rename = "owner-local-part")]
        #[serde(default)]
        owner_local_part: Option<String>,
        #[serde(rename = "request-local-part")]
        #[serde(default)]
        request_local_part: Option<String>,
        #[serde(rename = "archive-url")]
        #[serde(default)]
        archive_url: Option<String>,
    },
    AcceptSubscriptionRequest {
        pk: IntPOST,
    },
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum PostPolicySettings {
    AnnounceOnly,
    SubscriptionOnly,
    ApprovalNeeded,
    Open,
    Custom,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum SubscriptionPolicySettings {
    Open,
    Manual,
    Request,
    Custom,
}

/// Raw post page.
pub async fn list_post_raw(
    ListPostRawPath(id, msg_id): ListPostRawPath,
    State(state): State<Arc<AppState>>,
) -> Result<String, ResponseError> {
    let db = Connection::open_db(state.conf.clone())?.trusted();
    let Some(list) = (match id {
        ListPathIdentifier::Pk(id) => db.list(id)?,
        ListPathIdentifier::Id(id) => db.list_by_id(id)?,
    }) else {
        return Err(ResponseError::new(
            "List not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };

    let post = if let Some(post) = db.list_post_by_message_id(list.pk, &msg_id)? {
        post
    } else {
        return Err(ResponseError::new(
            format!("Post with Message-ID {} not found", msg_id),
            StatusCode::NOT_FOUND,
        ));
    };
    Ok(String::from_utf8_lossy(&post.message).to_string())
}

/// .eml post page.
pub async fn list_post_eml(
    ListPostEmlPath(id, msg_id): ListPostEmlPath,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ResponseError> {
    let db = Connection::open_db(state.conf.clone())?.trusted();
    let Some(list) = (match id {
        ListPathIdentifier::Pk(id) => db.list(id)?,
        ListPathIdentifier::Id(id) => db.list_by_id(id)?,
    }) else {
        return Err(ResponseError::new(
            "List not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };

    let post = if let Some(post) = db.list_post_by_message_id(list.pk, &msg_id)? {
        post
    } else {
        return Err(ResponseError::new(
            format!("Post with Message-ID {} not found", msg_id),
            StatusCode::NOT_FOUND,
        ));
    };
    let mut response = post.into_inner().message.into_response();
    response.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/octet-stream"),
    );
    response.headers_mut().insert(
        http::header::CONTENT_DISPOSITION,
        http::HeaderValue::try_from(format!(
            "attachment; filename=\"{}.eml\"",
            msg_id.trim().strip_carets()
        ))
        .unwrap(),
    );

    Ok(response)
}

pub async fn list_subscribers(
    ListEditSubscribersPath(id): ListEditSubscribersPath,
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
            "Not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };
    let list_owners = db.list_owners(list.pk)?;
    let user_address = &auth.current_user.as_ref().unwrap().address;
    if !list_owners.iter().any(|o| &o.address == user_address) {
        return Err(ResponseError::new(
            "Not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };

    let subs = {
        let mut stmt = db
            .connection
            .prepare("SELECT * FROM subscription WHERE list = ?;")?;
        let iter = stmt.query_map([&list.pk], |row| {
            let address: String = row.get("address")?;
            let name: Option<String> = row.get("name")?;
            let enabled: bool = row.get("enabled")?;
            let verified: bool = row.get("verified")?;
            let digest: bool = row.get("digest")?;
            let hide_address: bool = row.get("hide_address")?;
            let receive_duplicates: bool = row.get("receive_duplicates")?;
            let receive_own_posts: bool = row.get("receive_own_posts")?;
            let receive_confirmation: bool = row.get("receive_confirmation")?;
            //let last_digest: i64 = row.get("last_digest")?;
            let created: i64 = row.get("created")?;
            let last_modified: i64 = row.get("last_modified")?;
            Ok(minijinja::context! {
                address,
                name,
                enabled,
                verified,
                digest,
                hide_address,
                receive_duplicates,
                receive_own_posts,
                receive_confirmation,
                //last_digest => chrono::Utc.timestamp_opt(last_digest, 0).unwrap().to_string(),
                created => chrono::Utc.timestamp_opt(created, 0).unwrap().to_string(),
                last_modified => chrono::Utc.timestamp_opt(last_modified, 0).unwrap().to_string(),
            })
        })?;
        let mut ret = vec![];
        for el in iter {
            let el = el?;
            ret.push(el);
        }
        ret
    };

    let crumbs = vec![
        Crumb {
            label: "Home".into(),
            url: "/".into(),
        },
        Crumb {
            label: list.name.clone().into(),
            url: ListPath(list.id.to_string().into()).to_crumb(),
        },
        Crumb {
            label: format!("Edit {}", list.name).into(),
            url: ListEditPath(ListPathIdentifier::from(list.id.clone())).to_crumb(),
        },
        Crumb {
            label: format!("Subscribers of {}", list.name).into(),
            url: ListEditSubscribersPath(list.id.to_string().into()).to_crumb(),
        },
    ];
    let list_owners = db.list_owners(list.pk)?;
    let mut list_obj = MailingList::from(list.clone());
    list_obj.set_safety(list_owners.as_slice(), &state.conf.administrators);
    let context = minijinja::context! {
        canonical_url => ListEditSubscribersPath(ListPathIdentifier::from(list.id.clone())).to_crumb(),
        page_title => format!("Subscribers of {}", list.name),
        subs,
        list => Value::from_object(list_obj),
        current_user => auth.current_user,
        messages => session.drain_messages(),
        crumbs,
    };
    Ok(Html(
        TEMPLATES.get_template("lists/subs.html")?.render(context)?,
    ))
}

pub async fn list_candidates(
    ListEditCandidatesPath(id): ListEditCandidatesPath,
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
            "Not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };
    let list_owners = db.list_owners(list.pk)?;
    let user_address = &auth.current_user.as_ref().unwrap().address;
    if !list_owners.iter().any(|o| &o.address == user_address) {
        return Err(ResponseError::new(
            "Not found".to_string(),
            StatusCode::NOT_FOUND,
        ));
    };

    let subs = {
        let mut stmt = db
            .connection
            .prepare("SELECT * FROM candidate_subscription WHERE list = ?;")?;
        let iter = stmt.query_map([&list.pk], |row| {
            let pk: i64 = row.get("pk")?;
            let address: String = row.get("address")?;
            let name: Option<String> = row.get("name")?;
            let accepted: Option<i64> = row.get("accepted")?;
            let created: i64 = row.get("created")?;
            let last_modified: i64 = row.get("last_modified")?;
            Ok(minijinja::context! {
                pk,
                address,
                name,
                accepted => accepted.is_some(),
                created => chrono::Utc.timestamp_opt(created, 0).unwrap().to_string(),
                last_modified => chrono::Utc.timestamp_opt(last_modified, 0).unwrap().to_string(),
            })
        })?;
        let mut ret = vec![];
        for el in iter {
            let el = el?;
            ret.push(el);
        }
        ret
    };

    let crumbs = vec![
        Crumb {
            label: "Home".into(),
            url: "/".into(),
        },
        Crumb {
            label: list.name.clone().into(),
            url: ListPath(list.id.to_string().into()).to_crumb(),
        },
        Crumb {
            label: format!("Edit {}", list.name).into(),
            url: ListEditPath(ListPathIdentifier::from(list.id.clone())).to_crumb(),
        },
        Crumb {
            label: format!("Requests of {}", list.name).into(),
            url: ListEditCandidatesPath(list.id.to_string().into()).to_crumb(),
        },
    ];
    let mut list_obj: MailingList = MailingList::from(list.clone());
    list_obj.set_safety(list_owners.as_slice(), &state.conf.administrators);
    let context = minijinja::context! {
        canonical_url => ListEditCandidatesPath(ListPathIdentifier::from(list.id.clone())).to_crumb(),
        page_title => format!("Requests of {}", list.name),
        subs,
        list => Value::from_object(list_obj),
        current_user => auth.current_user,
        messages => session.drain_messages(),
        crumbs,
    };
    Ok(Html(
        TEMPLATES
            .get_template("lists/sub-requests.html")?
            .render(context)?,
    ))
}
