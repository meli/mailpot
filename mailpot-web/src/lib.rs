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

#![deny(
    //missing_docs,
    rustdoc::broken_intra_doc_links,
    /* groups */
    clippy::correctness,
    clippy::suspicious,
    clippy::complexity,
    clippy::perf,
    clippy::style,
    clippy::cargo,
    clippy::nursery,
    /* restriction */
    clippy::dbg_macro,
    clippy::rc_buffer,
    clippy::as_underscore,
    clippy::assertions_on_result_states,
    /* pedantic */
    clippy::cast_lossless,
    clippy::cast_possible_wrap,
    clippy::ptr_as_ptr,
    clippy::bool_to_int_with_if,
    clippy::borrow_as_ptr,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::cast_lossless,
    clippy::cast_ptr_alignment,
    clippy::naive_bytecount
)]
#![allow(clippy::multiple_crate_versions, clippy::missing_const_for_fn)]

pub use axum::{
    extract::{Path, Query, State},
    handler::Handler,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Extension, Form, Router,
};
pub use axum_extra::routing::TypedPath;
pub use axum_login::{
    memory_store::MemoryStore as AuthMemoryStore, secrecy::SecretVec, AuthLayer, AuthUser,
    RequireAuthorizationLayer,
};
pub use axum_sessions::{
    async_session::MemoryStore,
    extractors::{ReadableSession, WritableSession},
    SessionLayer,
};

pub type AuthContext =
    axum_login::extractors::AuthContext<i64, auth::User, Arc<AppState>, auth::Role>;

pub type RequireAuth = auth::auth_request::RequireAuthorizationLayer<i64, auth::User, auth::Role>;

pub use std::result::Result;
use std::{borrow::Cow, collections::HashMap, sync::Arc};

use chrono::Datelike;
pub use http::{Request, Response, StatusCode};
use mailpot::{models::DbVal, rusqlite::OptionalExtension, *};
use minijinja::{
    value::{Object, Value},
    Environment, Error,
};
use tokio::sync::RwLock;

pub mod auth;
pub mod cal;
pub mod help;
pub mod lists;
pub mod minijinja_utils;
pub mod settings;
pub mod topics;
pub mod typed_paths;
pub mod utils;

pub use auth::*;
pub use help::*;
pub use lists::{
    list, list_candidates, list_edit, list_edit_POST, list_post, list_post_eml, list_post_raw,
    list_subscribers, PostPolicySettings, SubscriptionPolicySettings,
};
pub use minijinja_utils::*;
pub use settings::{
    settings, settings_POST, user_list_subscription, user_list_subscription_POST,
    SubscriptionFormPayload,
};
pub use topics::*;
pub use typed_paths::{tsr::RouterExt, *};
pub use utils::*;

#[derive(Debug)]
pub struct ResponseError {
    pub inner: Box<dyn std::error::Error>,
    pub status: StatusCode,
}

impl std::fmt::Display for ResponseError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "Inner: {}, status: {}", self.inner, self.status)
    }
}

impl ResponseError {
    pub fn new(msg: String, status: StatusCode) -> Self {
        Self {
            inner: Box::<dyn std::error::Error + Send + Sync>::from(msg),
            status,
        }
    }
}

impl<E: Into<Box<dyn std::error::Error>>> From<E> for ResponseError {
    fn from(err: E) -> Self {
        Self {
            inner: err.into(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

pub trait IntoResponseError {
    fn with_status(self, status: StatusCode) -> ResponseError;
}

impl<E: Into<Box<dyn std::error::Error>>> IntoResponseError for E {
    fn with_status(self, status: StatusCode) -> ResponseError {
        ResponseError {
            status,
            ..ResponseError::from(self)
        }
    }
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> axum::response::Response {
        let Self { inner, status } = self;
        (status, inner.to_string()).into_response()
    }
}

pub trait IntoResponseErrorResult<R> {
    fn with_status(self, status: StatusCode) -> std::result::Result<R, ResponseError>;
}

impl<R, E> IntoResponseErrorResult<R> for std::result::Result<R, E>
where
    E: IntoResponseError,
{
    fn with_status(self, status: StatusCode) -> std::result::Result<R, ResponseError> {
        self.map_err(|err| err.with_status(status))
    }
}

#[derive(Clone)]
pub struct AppState {
    pub conf: Configuration,
    pub root_url_prefix: Value,
    pub public_url: String,
    pub site_title: Cow<'static, str>,
    pub site_subtitle: Option<Cow<'static, str>>,
    pub user_store: Arc<RwLock<HashMap<i64, User>>>,
    // ...
}

mod auth_impls {
    use super::*;
    type UserId = i64;
    type User = auth::User;
    type Role = auth::Role;

    impl AppState {
        pub async fn insert_user(&self, pk: UserId, user: User) {
            self.user_store.write().await.insert(pk, user);
        }
    }

    #[axum::async_trait]
    impl axum_login::UserStore<UserId, Role> for Arc<AppState>
    where
        User: axum_login::AuthUser<UserId, Role>,
    {
        type User = User;

        async fn load_user(
            &self,
            user_id: &UserId,
        ) -> std::result::Result<Option<Self::User>, eyre::Report> {
            Ok(self.user_store.read().await.get(user_id).cloned())
        }
    }
}

const fn _get_package_git_sha() -> Option<&'static str> {
    option_env!("PACKAGE_GIT_SHA")
}

const _PACKAGE_COMMIT_SHA: Option<&str> = _get_package_git_sha();

pub fn get_git_sha() -> std::borrow::Cow<'static, str> {
    if let Some(r) = _PACKAGE_COMMIT_SHA {
        return r.into();
    }
    build_info::build_info!(fn build_info);
    let info = build_info();
    info.version_control
        .as_ref()
        .and_then(|v| v.git())
        .map(|g| g.commit_short_id.clone())
        .map_or_else(|| "<unknown>".into(), |v| v.into())
}

pub const VERSION_INFO: &str = build_info::format!("{}", $.crate_info.version);
pub const BUILD_INFO: &str = build_info::format!("{}\t{}\t{}\t{}", $.crate_info.version, $.compiler, $.timestamp, $.crate_info.enabled_features);
pub const CLI_INFO: &str = build_info::format!("{} Version: {}\nAuthors: {}\nLicense: AGPL version 3 or later\nCompiler: {}\nBuild-Date: {}\nEnabled-features: {}", $.crate_info.name, $.crate_info.version, $.crate_info.authors, $.compiler, $.timestamp, $.crate_info.enabled_features);
