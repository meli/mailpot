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

use mailpot::models::{
    changesets::{AccountChangeset, ListSubscriptionChangeset},
    ListSubscription,
};

use super::*;

pub async fn settings(
    _: SettingsPath,
    mut session: WritableSession,
    Extension(user): Extension<User>,
    state: Arc<AppState>,
) -> Result<Html<String>, ResponseError> {
    let crumbs = vec![
        Crumb {
            label: "Home".into(),
            url: "/".into(),
        },
        Crumb {
            label: "Settings".into(),
            url: SettingsPath.to_crumb(),
        },
    ];
    let db = Connection::open_db(state.conf.clone())?;
    let acc = db
        .account_by_address(&user.address)
        .with_status(StatusCode::BAD_REQUEST)?
        .ok_or_else(|| {
            ResponseError::new("Account not found".to_string(), StatusCode::BAD_REQUEST)
        })?;
    let subscriptions = db
        .account_subscriptions(acc.pk())
        .with_status(StatusCode::BAD_REQUEST)?
        .into_iter()
        .filter_map(|s| match db.list(s.list) {
            Err(err) => Some(Err(err)),
            Ok(Some(list)) => Some(Ok((s, list))),
            Ok(None) => None,
        })
        .collect::<Result<
            Vec<(
                DbVal<mailpot::models::ListSubscription>,
                DbVal<mailpot::models::MailingList>,
            )>,
            mailpot::Error,
        >>()?;

    let context = minijinja::context! {
        page_title => "Account settings",
        user => user,
        subscriptions => subscriptions,
        current_user => user,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Ok(Html(
        TEMPLATES.get_template("settings.html")?.render(context)?,
    ))
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ChangeSetting {
    Subscribe { list_pk: IntPOST },
    Unsubscribe { list_pk: IntPOST },
    ChangePassword { new: String },
    ChangePublicKey { new: String },
    // RemovePassword,
    RemovePublicKey,
    ChangeName { new: String },
}

#[allow(non_snake_case)]
pub async fn settings_POST(
    _: SettingsPath,
    mut session: WritableSession,
    Extension(user): Extension<User>,
    Form(payload): Form<ChangeSetting>,
    state: Arc<AppState>,
) -> Result<Redirect, ResponseError> {
    let db = Connection::open_db(state.conf.clone())?;
    let acc = db
        .account_by_address(&user.address)
        .with_status(StatusCode::BAD_REQUEST)?
        .ok_or_else(|| {
            ResponseError::new("Account not found".to_string(), StatusCode::BAD_REQUEST)
        })?;

    match payload {
        ChangeSetting::Subscribe {
            list_pk: IntPOST(list_pk),
        } => {
            let subscriptions = db
                .account_subscriptions(acc.pk())
                .with_status(StatusCode::BAD_REQUEST)?;
            if subscriptions.iter().any(|s| s.list == list_pk) {
                session.add_message(Message {
                    message: "You are already subscribed to this list.".into(),
                    level: Level::Info,
                })?;
            } else {
                db.add_subscription(
                    list_pk,
                    ListSubscription {
                        pk: 0,
                        list: list_pk,
                        account: Some(acc.pk()),
                        address: acc.address.clone(),
                        name: acc.name.clone(),
                        digest: false,
                        enabled: true,
                        verified: true,
                        hide_address: false,
                        receive_duplicates: false,
                        receive_own_posts: false,
                        receive_confirmation: false,
                    },
                )?;
                session.add_message(Message {
                    message: "You have subscribed to this list.".into(),
                    level: Level::Success,
                })?;
            }
        }
        ChangeSetting::Unsubscribe {
            list_pk: IntPOST(list_pk),
        } => {
            let subscriptions = db
                .account_subscriptions(acc.pk())
                .with_status(StatusCode::BAD_REQUEST)?;
            if !subscriptions.iter().any(|s| s.list == list_pk) {
                session.add_message(Message {
                    message: "You are already not subscribed to this list.".into(),
                    level: Level::Info,
                })?;
            } else {
                let db = db.trusted();
                db.remove_subscription(list_pk, &acc.address)?;
                session.add_message(Message {
                    message: "You have unsubscribed from this list.".into(),
                    level: Level::Success,
                })?;
            }
        }
        ChangeSetting::ChangePassword { new } => {
            db.update_account(AccountChangeset {
                address: acc.address.clone(),
                name: None,
                public_key: None,
                password: Some(new.clone()),
                enabled: None,
            })
            .with_status(StatusCode::BAD_REQUEST)?;
            session.add_message(Message {
                message: "You have successfully updated your SSH public key.".into(),
                level: Level::Success,
            })?;
            let mut user = user.clone();
            user.password = new;
            state.insert_user(acc.pk(), user).await;
        }
        ChangeSetting::ChangePublicKey { new } => {
            db.update_account(AccountChangeset {
                address: acc.address.clone(),
                name: None,
                public_key: Some(Some(new.clone())),
                password: None,
                enabled: None,
            })
            .with_status(StatusCode::BAD_REQUEST)?;
            session.add_message(Message {
                message: "You have successfully updated your PGP public key.".into(),
                level: Level::Success,
            })?;
            let mut user = user.clone();
            user.public_key = Some(new);
            state.insert_user(acc.pk(), user).await;
        }
        ChangeSetting::RemovePublicKey => {
            db.update_account(AccountChangeset {
                address: acc.address.clone(),
                name: None,
                public_key: Some(None),
                password: None,
                enabled: None,
            })
            .with_status(StatusCode::BAD_REQUEST)?;
            session.add_message(Message {
                message: "You have successfully removed your PGP public key.".into(),
                level: Level::Success,
            })?;
            let mut user = user.clone();
            user.public_key = None;
            state.insert_user(acc.pk(), user).await;
        }
        ChangeSetting::ChangeName { new } => {
            let new = if new.trim().is_empty() {
                None
            } else {
                Some(new)
            };
            db.update_account(AccountChangeset {
                address: acc.address.clone(),
                name: Some(new.clone()),
                public_key: None,
                password: None,
                enabled: None,
            })
            .with_status(StatusCode::BAD_REQUEST)?;
            session.add_message(Message {
                message: "You have successfully updated your name.".into(),
                level: Level::Success,
            })?;
            let mut user = user.clone();
            user.name = new.clone();
            state.insert_user(acc.pk(), user).await;
        }
    }

    Ok(Redirect::to(&format!(
        "{}{}",
        &state.root_url_prefix,
        SettingsPath.to_uri()
    )))
}

pub async fn user_list_subscription(
    ListSettingsPath(id): ListSettingsPath,
    mut session: WritableSession,
    Extension(user): Extension<User>,
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
    let acc = match db.account_by_address(&user.address)? {
        Some(v) => v,
        None => {
            return Err(ResponseError::new(
                "Account not found".to_string(),
                StatusCode::BAD_REQUEST,
            ))
        }
    };
    let mut subscriptions = db
        .account_subscriptions(acc.pk())
        .with_status(StatusCode::BAD_REQUEST)?;
    subscriptions.retain(|s| s.list == list.pk());
    let subscription = db
        .list_subscription(
            list.pk(),
            subscriptions
                .get(0)
                .ok_or_else(|| {
                    ResponseError::new(
                        "Subscription not found".to_string(),
                        StatusCode::BAD_REQUEST,
                    )
                })?
                .pk(),
        )
        .with_status(StatusCode::BAD_REQUEST)?;

    let crumbs = vec![
        Crumb {
            label: "Home".into(),
            url: "/".into(),
        },
        Crumb {
            label: "Settings".into(),
            url: SettingsPath.to_crumb(),
        },
        Crumb {
            label: "List Subscription".into(),
            url: ListSettingsPath(list.pk().into()).to_crumb(),
        },
    ];

    let context = minijinja::context! {
        page_title => "Subscription settings",
        user => user,
        list => list,
        subscription => subscription,
        current_user => user,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Ok(Html(
        TEMPLATES
            .get_template("settings_subscription.html")?
            .render(context)?,
    ))
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct SubscriptionFormPayload {
    #[serde(default)]
    pub digest: bool,
    #[serde(default)]
    pub hide_address: bool,
    #[serde(default)]
    pub receive_duplicates: bool,
    #[serde(default)]
    pub receive_own_posts: bool,
    #[serde(default)]
    pub receive_confirmation: bool,
}

#[allow(non_snake_case)]
pub async fn user_list_subscription_POST(
    ListSettingsPath(id): ListSettingsPath,
    mut session: WritableSession,
    Extension(user): Extension<User>,
    Form(payload): Form<SubscriptionFormPayload>,
    state: Arc<AppState>,
) -> Result<Redirect, ResponseError> {
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

    let acc = match db.account_by_address(&user.address)? {
        Some(v) => v,
        None => {
            return Err(ResponseError::new(
                "Account with this address was not found".to_string(),
                StatusCode::BAD_REQUEST,
            ));
        }
    };
    let mut subscriptions = db
        .account_subscriptions(acc.pk())
        .with_status(StatusCode::BAD_REQUEST)?;

    subscriptions.retain(|s| s.list == list.pk());
    let mut s = db
        .list_subscription(list.pk(), subscriptions[0].pk())
        .with_status(StatusCode::BAD_REQUEST)?;

    let SubscriptionFormPayload {
        digest,
        hide_address,
        receive_duplicates,
        receive_own_posts,
        receive_confirmation,
    } = payload;

    let cset = ListSubscriptionChangeset {
        list: s.list,
        address: std::mem::take(&mut s.address),
        account: None,
        name: None,
        digest: Some(digest),
        hide_address: Some(hide_address),
        receive_duplicates: Some(receive_duplicates),
        receive_own_posts: Some(receive_own_posts),
        receive_confirmation: Some(receive_confirmation),
        enabled: None,
        verified: None,
    };

    db.update_subscription(cset)
        .with_status(StatusCode::BAD_REQUEST)?;

    session.add_message(Message {
        message: "Settings saved successfully.".into(),
        level: Level::Success,
    })?;

    Ok(Redirect::to(&format!(
        "{}{}",
        &state.root_url_prefix,
        ListSettingsPath(list.id.clone().into()).to_uri()
    )))
}
