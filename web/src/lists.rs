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
            "SELECT count(*) FROM candidate_subscription WHERE list = ? AND accepted IS NOT NULL;",
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
            url: ListPath(list.pk().into()).to_crumb(),
        },
    ];
    let context = minijinja::context! {
        title => state.site_title.as_ref(),
        page_title => format!("Edit {} settings", list.name),
        description => &list.description,
        post_policy => &post_policy,
        subscription_policy => &subscription_policy,
        list_owners => list_owners,
        post_count => post_count,
        subs_count => subs_count,
        sub_requests_count => sub_requests_count,
        root_url_prefix => &state.root_url_prefix,
        list => Value::from_object(MailingList::from(list)),
        current_user => auth.current_user,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Ok(Html(
        TEMPLATES.get_template("lists/edit.html")?.render(context)?,
    ))
}

pub async fn list_edit_post(
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

    let mut db = db.trusted();
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

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct MetadataSettings {}
