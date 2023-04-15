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

pub use axum::{
    extract::{Path, Query, State},
    handler::Handler,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Extension, Form, Router,
};

pub use axum_extra::routing::RouterExt;

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

pub use http::{Request, Response, StatusCode};

use chrono::Datelike;
use minijinja::value::{Object, Value};
use minijinja::{Environment, Error, Source};

use std::borrow::Cow;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub use mailpot::models::DbVal;
pub use mailpot::*;
pub use std::result::Result;

pub mod auth;
pub mod cal;
pub mod settings;
pub mod utils;

pub use auth::*;
pub use cal::calendarize;
pub use cal::*;
pub use settings::*;
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
    pub root_url_prefix: String,
    pub public_url: String,
    pub site_title: Cow<'static, str>,
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
