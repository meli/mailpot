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

pub use mailpot::PATH_SEGMENT;
use percent_encoding::utf8_percent_encode;

use super::*;

pub trait IntoCrumb: TypedPath {
    fn to_crumb(&self) -> Cow<'static, str> {
        Cow::from(self.to_uri().to_string())
    }
}

impl<TP: TypedPath> IntoCrumb for TP {}

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum ListPathIdentifier {
    Pk(#[serde(deserialize_with = "parse_int")] i64),
    Id(String),
}

fn parse_int<'de, T, D>(de: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    use serde::Deserialize;
    String::deserialize(de)?
        .parse()
        .map_err(serde::de::Error::custom)
}

impl From<i64> for ListPathIdentifier {
    fn from(val: i64) -> Self {
        Self::Pk(val)
    }
}

impl From<String> for ListPathIdentifier {
    fn from(val: String) -> Self {
        Self::Id(val)
    }
}

impl std::fmt::Display for ListPathIdentifier {
    #[allow(clippy::unnecessary_to_owned)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let id: Cow<'_, str> = match self {
            Self::Pk(id) => id.to_string().into(),
            Self::Id(id) => id.into(),
        };
        write!(f, "{}", utf8_percent_encode(&id, PATH_SEGMENT,))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/list/:id/")]
pub struct ListPath(pub ListPathIdentifier);

impl From<&DbVal<mailpot::models::MailingList>> for ListPath {
    fn from(val: &DbVal<mailpot::models::MailingList>) -> Self {
        Self(ListPathIdentifier::Id(val.id.clone()))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/list/:id/posts/:msgid/")]
pub struct ListPostPath(pub ListPathIdentifier, pub String);

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/list/:id/posts/:msgid/raw/")]
pub struct ListPostRawPath(pub ListPathIdentifier, pub String);

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/list/:id/posts/:msgid/eml/")]
pub struct ListPostEmlPath(pub ListPathIdentifier, pub String);

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/list/:id/edit/")]
pub struct ListEditPath(pub ListPathIdentifier);

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/list/:id/edit/subscribers/")]
pub struct ListEditSubscribersPath(pub ListPathIdentifier);

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/list/:id/edit/candidates/")]
pub struct ListEditCandidatesPath(pub ListPathIdentifier);

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/settings/list/:id/")]
pub struct ListSettingsPath(pub ListPathIdentifier);

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/login/")]
pub struct LoginPath;

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/logout/")]
pub struct LogoutPath;

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/settings/")]
pub struct SettingsPath;

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/help/")]
pub struct HelpPath;

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize, TypedPath)]
#[typed_path("/topics/")]
pub struct TopicsPath;

macro_rules! unit_impl {
    ($ident:ident, $ty:expr) => {
        pub fn $ident(state: &minijinja::State) -> std::result::Result<Value, Error> {
            urlize(state, Value::from($ty.to_crumb().to_string()))
        }
    };
}

unit_impl!(login_path, LoginPath);
unit_impl!(logout_path, LogoutPath);
unit_impl!(settings_path, SettingsPath);
unit_impl!(help_path, HelpPath);

macro_rules! list_id_impl {
    ($ident:ident, $ty:tt) => {
        pub fn $ident(state: &minijinja::State, id: Value) -> std::result::Result<Value, Error> {
            urlize(
                state,
                if let Some(id) = id.as_str() {
                    Value::from(
                        $ty(ListPathIdentifier::Id(id.to_string()))
                            .to_crumb()
                            .to_string(),
                    )
                } else {
                    let pk = id.try_into()?;
                    Value::from($ty(ListPathIdentifier::Pk(pk)).to_crumb().to_string())
                },
            )
        }
    };
}

list_id_impl!(list_path, ListPath);
list_id_impl!(list_settings_path, ListSettingsPath);
list_id_impl!(list_edit_path, ListEditPath);
list_id_impl!(list_subscribers_path, ListEditSubscribersPath);
list_id_impl!(list_candidates_path, ListEditCandidatesPath);

macro_rules! list_post_impl {
    ($ident:ident, $ty:tt) => {
        pub fn $ident(state: &minijinja::State, id: Value, msg_id: Value) -> std::result::Result<Value, Error> {
            urlize(state, {
                let Some(msg_id) = msg_id.as_str().map(|s| if s.starts_with('<') && s.ends_with('>') { s.to_string() } else {
                    format!("<{s}>")
                }) else {
                    return Err(Error::new(
                            minijinja::ErrorKind::UnknownMethod,
                            "Second argument of list_post_path must be a string."
                    ));
                };

                if let Some(id) = id.as_str() {
                    Value::from(
                        $ty(ListPathIdentifier::Id(id.to_string()), msg_id)
                        .to_crumb()
                        .to_string(),
                    )
                } else {
                    let pk = id.try_into()?;
                    Value::from(
                        $ty(ListPathIdentifier::Pk(pk), msg_id)
                        .to_crumb()
                        .to_string(),
                    )
                }
            })
        }
    };
}

list_post_impl!(list_post_path, ListPostPath);
list_post_impl!(post_raw_path, ListPostRawPath);
list_post_impl!(post_eml_path, ListPostEmlPath);

pub mod tsr {
    use std::{borrow::Cow, convert::Infallible};

    use axum::{
        http::Request,
        response::{IntoResponse, Redirect, Response},
        routing::{any, MethodRouter},
        Router,
    };
    use axum_extra::routing::{RouterExt as ExtraRouterExt, SecondElementIs, TypedPath};
    use http::{uri::PathAndQuery, StatusCode, Uri};
    use tower_service::Service;

    /// Extension trait that adds additional methods to [`Router`].
    pub trait RouterExt<S, B>: ExtraRouterExt<S, B> {
        /// Add a typed `GET` route to the router.
        ///
        /// The path will be inferred from the first argument to the handler
        /// function which must implement [`TypedPath`].
        ///
        /// See [`TypedPath`] for more details and examples.
        fn typed_get<H, T, P>(self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath;

        /// Add a typed `DELETE` route to the router.
        ///
        /// The path will be inferred from the first argument to the handler
        /// function which must implement [`TypedPath`].
        ///
        /// See [`TypedPath`] for more details and examples.
        fn typed_delete<H, T, P>(self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath;

        /// Add a typed `HEAD` route to the router.
        ///
        /// The path will be inferred from the first argument to the handler
        /// function which must implement [`TypedPath`].
        ///
        /// See [`TypedPath`] for more details and examples.
        fn typed_head<H, T, P>(self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath;

        /// Add a typed `OPTIONS` route to the router.
        ///
        /// The path will be inferred from the first argument to the handler
        /// function which must implement [`TypedPath`].
        ///
        /// See [`TypedPath`] for more details and examples.
        fn typed_options<H, T, P>(self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath;

        /// Add a typed `PATCH` route to the router.
        ///
        /// The path will be inferred from the first argument to the handler
        /// function which must implement [`TypedPath`].
        ///
        /// See [`TypedPath`] for more details and examples.
        fn typed_patch<H, T, P>(self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath;

        /// Add a typed `POST` route to the router.
        ///
        /// The path will be inferred from the first argument to the handler
        /// function which must implement [`TypedPath`].
        ///
        /// See [`TypedPath`] for more details and examples.
        fn typed_post<H, T, P>(self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath;

        /// Add a typed `PUT` route to the router.
        ///
        /// The path will be inferred from the first argument to the handler
        /// function which must implement [`TypedPath`].
        ///
        /// See [`TypedPath`] for more details and examples.
        fn typed_put<H, T, P>(self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath;

        /// Add a typed `TRACE` route to the router.
        ///
        /// The path will be inferred from the first argument to the handler
        /// function which must implement [`TypedPath`].
        ///
        /// See [`TypedPath`] for more details and examples.
        fn typed_trace<H, T, P>(self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath;

        /// Add another route to the router with an additional "trailing slash
        /// redirect" route.
        ///
        /// If you add a route _without_ a trailing slash, such as `/foo`, this
        /// method will also add a route for `/foo/` that redirects to
        /// `/foo`.
        ///
        /// If you add a route _with_ a trailing slash, such as `/bar/`, this
        /// method will also add a route for `/bar` that redirects to
        /// `/bar/`.
        ///
        /// This is similar to what axum 0.5.x did by default, except this
        /// explicitly adds another route, so trying to add a `/foo/`
        /// route after calling `.route_with_tsr("/foo", /* ... */)`
        /// will result in a panic due to route overlap.
        ///
        /// # Example
        ///
        /// ```
        /// use axum::{routing::get, Router};
        /// use axum_extra::routing::RouterExt;
        ///
        /// let app = Router::new()
        ///     // `/foo/` will redirect to `/foo`
        ///     .route_with_tsr("/foo", get(|| async {}))
        ///     // `/bar` will redirect to `/bar/`
        ///     .route_with_tsr("/bar/", get(|| async {}));
        /// # let _: Router = app;
        /// ```
        fn route_with_tsr(self, path: &str, method_router: MethodRouter<S, B>) -> Self
        where
            Self: Sized;

        /// Add another route to the router with an additional "trailing slash
        /// redirect" route.
        ///
        /// This works like [`RouterExt::route_with_tsr`] but accepts any
        /// [`Service`].
        fn route_service_with_tsr<T>(self, path: &str, service: T) -> Self
        where
            T: Service<Request<B>, Error = Infallible> + Clone + Send + 'static,
            T::Response: IntoResponse,
            T::Future: Send + 'static,
            Self: Sized;
    }

    impl<S, B> RouterExt<S, B> for Router<S, B>
    where
        B: axum::body::HttpBody + Send + 'static,
        S: Clone + Send + Sync + 'static,
    {
        fn typed_get<H, T, P>(mut self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath,
        {
            let (tsr_path, tsr_handler) = tsr_redirect_route(P::PATH);
            self = self.route(
                tsr_path.as_ref(),
                axum::routing::get(move |url| tsr_handler_into_async(url, tsr_handler)),
            );
            self = self.route(P::PATH, axum::routing::get(handler));
            self
        }

        fn typed_delete<H, T, P>(mut self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath,
        {
            let (tsr_path, tsr_handler) = tsr_redirect_route(P::PATH);
            self = self.route(
                tsr_path.as_ref(),
                axum::routing::delete(move |url| tsr_handler_into_async(url, tsr_handler)),
            );
            self = self.route(P::PATH, axum::routing::delete(handler));
            self
        }

        fn typed_head<H, T, P>(mut self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath,
        {
            let (tsr_path, tsr_handler) = tsr_redirect_route(P::PATH);
            self = self.route(
                tsr_path.as_ref(),
                axum::routing::head(move |url| tsr_handler_into_async(url, tsr_handler)),
            );
            self = self.route(P::PATH, axum::routing::head(handler));
            self
        }

        fn typed_options<H, T, P>(mut self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath,
        {
            let (tsr_path, tsr_handler) = tsr_redirect_route(P::PATH);
            self = self.route(
                tsr_path.as_ref(),
                axum::routing::options(move |url| tsr_handler_into_async(url, tsr_handler)),
            );
            self = self.route(P::PATH, axum::routing::options(handler));
            self
        }

        fn typed_patch<H, T, P>(mut self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath,
        {
            let (tsr_path, tsr_handler) = tsr_redirect_route(P::PATH);
            self = self.route(
                tsr_path.as_ref(),
                axum::routing::patch(move |url| tsr_handler_into_async(url, tsr_handler)),
            );
            self = self.route(P::PATH, axum::routing::patch(handler));
            self
        }

        fn typed_post<H, T, P>(mut self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath,
        {
            let (tsr_path, tsr_handler) = tsr_redirect_route(P::PATH);
            self = self.route(
                tsr_path.as_ref(),
                axum::routing::post(move |url| tsr_handler_into_async(url, tsr_handler)),
            );
            self = self.route(P::PATH, axum::routing::post(handler));
            self
        }

        fn typed_put<H, T, P>(mut self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath,
        {
            let (tsr_path, tsr_handler) = tsr_redirect_route(P::PATH);
            self = self.route(
                tsr_path.as_ref(),
                axum::routing::put(move |url| tsr_handler_into_async(url, tsr_handler)),
            );
            self = self.route(P::PATH, axum::routing::put(handler));
            self
        }

        fn typed_trace<H, T, P>(mut self, handler: H) -> Self
        where
            H: axum::handler::Handler<T, S, B>,
            T: SecondElementIs<P> + 'static,
            P: TypedPath,
        {
            let (tsr_path, tsr_handler) = tsr_redirect_route(P::PATH);
            self = self.route(
                tsr_path.as_ref(),
                axum::routing::trace(move |url| tsr_handler_into_async(url, tsr_handler)),
            );
            self = self.route(P::PATH, axum::routing::trace(handler));
            self
        }

        #[track_caller]
        fn route_with_tsr(mut self, path: &str, method_router: MethodRouter<S, B>) -> Self
        where
            Self: Sized,
        {
            validate_tsr_path(path);
            self = self.route(path, method_router);
            add_tsr_redirect_route(self, path)
        }

        #[track_caller]
        fn route_service_with_tsr<T>(mut self, path: &str, service: T) -> Self
        where
            T: Service<Request<B>, Error = Infallible> + Clone + Send + 'static,
            T::Response: IntoResponse,
            T::Future: Send + 'static,
            Self: Sized,
        {
            validate_tsr_path(path);
            self = self.route_service(path, service);
            add_tsr_redirect_route(self, path)
        }
    }

    #[track_caller]
    fn validate_tsr_path(path: &str) {
        if path == "/" {
            panic!("Cannot add a trailing slash redirect route for `/`")
        }
    }

    #[inline]
    fn add_tsr_redirect_route<S, B>(router: Router<S, B>, path: &str) -> Router<S, B>
    where
        B: axum::body::HttpBody + Send + 'static,
        S: Clone + Send + Sync + 'static,
    {
        async fn redirect_handler(uri: Uri) -> Response {
            let new_uri = map_path(uri, |path| {
                path.strip_suffix('/')
                    .map(Cow::Borrowed)
                    .unwrap_or_else(|| Cow::Owned(format!("{path}/")))
            });

            new_uri.map_or_else(
                || StatusCode::BAD_REQUEST.into_response(),
                |new_uri| Redirect::permanent(&new_uri.to_string()).into_response(),
            )
        }

        if let Some(path_without_trailing_slash) = path.strip_suffix('/') {
            router.route(path_without_trailing_slash, any(redirect_handler))
        } else {
            router.route(&format!("{path}/"), any(redirect_handler))
        }
    }

    #[inline]
    fn tsr_redirect_route(path: &'_ str) -> (Cow<'_, str>, fn(Uri) -> Response) {
        fn redirect_handler(uri: Uri) -> Response {
            let new_uri = map_path(uri, |path| {
                path.strip_suffix('/')
                    .map(Cow::Borrowed)
                    .unwrap_or_else(|| Cow::Owned(format!("{path}/")))
            });

            new_uri.map_or_else(
                || StatusCode::BAD_REQUEST.into_response(),
                |new_uri| Redirect::permanent(&new_uri.to_string()).into_response(),
            )
        }

        path.strip_suffix('/').map_or_else(
            || {
                (
                    Cow::Owned(format!("{path}/")),
                    redirect_handler as fn(Uri) -> Response,
                )
            },
            |path_without_trailing_slash| {
                (
                    Cow::Borrowed(path_without_trailing_slash),
                    redirect_handler as fn(Uri) -> Response,
                )
            },
        )
    }

    #[inline]
    async fn tsr_handler_into_async(u: Uri, h: fn(Uri) -> Response) -> Response {
        h(u)
    }

    /// Map the path of a `Uri`.
    ///
    /// Returns `None` if the `Uri` cannot be put back together with the new
    /// path.
    fn map_path<F>(original_uri: Uri, f: F) -> Option<Uri>
    where
        F: FnOnce(&str) -> Cow<'_, str>,
    {
        let mut parts = original_uri.into_parts();
        let path_and_query = parts.path_and_query.as_ref()?;

        let new_path = f(path_and_query.path());

        let new_path_and_query = if let Some(query) = &path_and_query.query() {
            format!("{new_path}?{query}").parse::<PathAndQuery>().ok()?
        } else {
            new_path.parse::<PathAndQuery>().ok()?
        };
        parts.path_and_query = Some(new_path_and_query);

        Uri::from_parts(parts).ok()
    }
}
