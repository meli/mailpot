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

use std::{borrow::Cow, process::Stdio};

use tempfile::NamedTempFile;
use tokio::{fs::File, io::AsyncWriteExt, process::Command};

use super::*;

const TOKEN_KEY: &str = "ssh_challenge";
const EXPIRY_IN_SECS: i64 = 6 * 60;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, Eq, PartialEq, PartialOrd)]
pub enum Role {
    User,
    Admin,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct User {
    /// SSH signature.
    pub ssh_signature: String,
    /// User role.
    pub role: Role,
    /// Database primary key.
    pub pk: i64,
    /// Accounts's display name, optional.
    pub name: Option<String>,
    /// Account's e-mail address.
    pub address: String,
    /// GPG public key.
    pub public_key: Option<String>,
    /// SSH public key.
    pub password: String,
    /// Whether this account is enabled.
    pub enabled: bool,
}

impl AuthUser<i64, Role> for User {
    fn get_id(&self) -> i64 {
        self.pk
    }

    fn get_password_hash(&self) -> SecretVec<u8> {
        SecretVec::new(self.ssh_signature.clone().into())
    }

    fn get_role(&self) -> Option<Role> {
        Some(self.role)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct AuthFormPayload {
    pub address: String,
    pub password: String,
}

pub async fn ssh_signin(
    _: LoginPath,
    mut session: WritableSession,
    Query(next): Query<Next>,
    auth: AuthContext,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if auth.current_user.is_some() {
        if let Err(err) = session.add_message(Message {
            message: "You are already logged in.".into(),
            level: Level::Info,
        }) {
            return err.into_response();
        }
        return next
            .or_else(|| format!("{}{}", state.root_url_prefix, SettingsPath.to_uri()))
            .into_response();
    }
    if next.next.is_some() {
        if let Err(err) = session.add_message(Message {
            message: "You need to be logged in to access this page.".into(),
            level: Level::Info,
        }) {
            return err.into_response();
        };
    }

    let now: i64 = chrono::offset::Utc::now().timestamp();

    let prev_token = if let Some(tok) = session.get::<(String, i64)>(TOKEN_KEY) {
        let timestamp: i64 = tok.1;
        if !(timestamp < now && now - timestamp < EXPIRY_IN_SECS) {
            session.remove(TOKEN_KEY);
            None
        } else {
            Some(tok)
        }
    } else {
        None
    };

    let (token, timestamp): (String, i64) = prev_token.map_or_else(
        || {
            use rand::{distributions::Alphanumeric, thread_rng, Rng};

            let mut rng = thread_rng();
            let chars: String = (0..7).map(|_| rng.sample(Alphanumeric) as char).collect();
            println!("Random chars: {}", chars);
            session.insert(TOKEN_KEY, (&chars, now)).unwrap();
            (chars, now)
        },
        |tok| tok,
    );
    let timeout_left = ((timestamp + EXPIRY_IN_SECS) - now) as f64 / 60.0;

    let crumbs = vec![
        Crumb {
            label: "Home".into(),
            url: "/".into(),
        },
        Crumb {
            label: "Sign in".into(),
            url: LoginPath.to_crumb(),
        },
    ];

    let context = minijinja::context! {
        namespace => &state.public_url,
        page_title => "Log in",
        ssh_challenge => token,
        timeout_left => timeout_left,
        current_user => auth.current_user,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Html(
        TEMPLATES
            .get_template("auth.html")
            .unwrap()
            .render(context)
            .unwrap_or_else(|err| err.to_string()),
    )
    .into_response()
}

#[allow(non_snake_case)]
pub async fn ssh_signin_POST(
    _: LoginPath,
    mut session: WritableSession,
    Query(next): Query<Next>,
    mut auth: AuthContext,
    Form(payload): Form<AuthFormPayload>,
    state: Arc<AppState>,
) -> Result<Redirect, ResponseError> {
    if auth.current_user.as_ref().is_some() {
        session.add_message(Message {
            message: "You are already logged in.".into(),
            level: Level::Info,
        })?;
        return Ok(next.or_else(|| format!("{}{}", state.root_url_prefix, SettingsPath.to_uri())));
    }

    let now: i64 = chrono::offset::Utc::now().timestamp();

    let (_prev_token, _) = if let Some(tok @ (_, timestamp)) =
        session.get::<(String, i64)>(TOKEN_KEY)
    {
        if !(timestamp <= now && now - timestamp < EXPIRY_IN_SECS) {
            session.add_message(Message {
                message: "The token has expired. Please retry.".into(),
                level: Level::Error,
            })?;
            return Ok(Redirect::to(&format!(
                "{}{}?next={}",
                state.root_url_prefix,
                LoginPath.to_uri(),
                next.next.as_ref().map_or(Cow::Borrowed(""), |next| format!(
                    "?next={}",
                    percent_encoding::utf8_percent_encode(
                        next.as_str(),
                        percent_encoding::CONTROLS
                    )
                )
                .into())
            )));
        } else {
            tok
        }
    } else {
        session.add_message(Message {
            message: "The token has expired. Please retry.".into(),
            level: Level::Error,
        })?;
        return Ok(Redirect::to(&format!(
            "{}{}{}",
            state.root_url_prefix,
            LoginPath.to_uri(),
            next.next.as_ref().map_or(Cow::Borrowed(""), |next| format!(
                "?next={}",
                percent_encoding::utf8_percent_encode(next.as_str(), percent_encoding::CONTROLS)
            )
            .into())
        )));
    };

    let db = Connection::open_db(state.conf.clone())?;
    let mut acc = match db
        .account_by_address(&payload.address)
        .with_status(StatusCode::BAD_REQUEST)?
    {
        Some(v) => v,
        None => {
            session.add_message(Message {
                message: "Invalid account details, please retry.".into(),
                level: Level::Error,
            })?;
            return Ok(Redirect::to(&format!(
                "{}{}{}",
                state.root_url_prefix,
                LoginPath.to_uri(),
                next.next.as_ref().map_or(Cow::Borrowed(""), |next| format!(
                    "?next={}",
                    percent_encoding::utf8_percent_encode(
                        next.as_str(),
                        percent_encoding::CONTROLS
                    )
                )
                .into())
            )));
        }
    };
    #[cfg(not(debug_assertions))]
    let sig = SshSignature {
        email: payload.address.clone(),
        ssh_public_key: acc.password.clone(),
        ssh_signature: payload.password.clone(),
        namespace: std::env::var("SSH_NAMESPACE")
            .unwrap_or_else(|_| "lists.mailpot.rs".to_string())
            .into(),
        token: _prev_token,
    };
    #[cfg(not(debug_assertions))]
    {
        #[cfg(not(feature = "ssh-key"))]
        let ssh_verify_fn = ssh_verify;
        #[cfg(feature = "ssh-key")]
        let ssh_verify_fn = ssh_verify_in_memory;
        if let Err(err) = ssh_verify_fn(sig).await {
            session.add_message(Message {
                message: format!("Could not verify signature: {err}").into(),
                level: Level::Error,
            })?;
            return Ok(Redirect::to(&format!(
                "{}{}{}",
                state.root_url_prefix,
                LoginPath.to_uri(),
                next.next.as_ref().map_or(Cow::Borrowed(""), |next| format!(
                    "?next={}",
                    percent_encoding::utf8_percent_encode(
                        next.as_str(),
                        percent_encoding::CONTROLS
                    )
                )
                .into())
            )));
        }
    }

    let user = User {
        pk: acc.pk(),
        ssh_signature: payload.password,
        role: if db
            .conf()
            .administrators
            .iter()
            .any(|a| a.eq_ignore_ascii_case(&payload.address))
        {
            Role::Admin
        } else {
            Role::User
        },
        public_key: std::mem::take(&mut acc.public_key),
        password: std::mem::take(&mut acc.password),
        name: std::mem::take(&mut acc.name),
        address: payload.address,
        enabled: acc.enabled,
    };
    state.insert_user(acc.pk(), user.clone()).await;
    drop(session);
    auth.login(&user)
        .await
        .map_err(|err| ResponseError::new(err.to_string(), StatusCode::BAD_REQUEST))?;
    Ok(next.or_else(|| format!("{}{}", state.root_url_prefix, SettingsPath.to_uri())))
}

#[derive(Debug, Clone, Default)]
pub struct SshSignature {
    pub email: String,
    pub ssh_public_key: String,
    pub ssh_signature: String,
    pub namespace: Cow<'static, str>,
    pub token: String,
}

/// Run ssh signature validation with `ssh-keygen` binary.
///
/// ```no_run
/// use mailpot_web::{ssh_verify, SshSignature};
///
/// async fn verify_signature(
///     ssh_public_key: String,
///     ssh_signature: String,
/// ) -> std::result::Result<(), Box<dyn std::error::Error>> {
///     let sig = SshSignature {
///         email: "user@example.com".to_string(),
///         ssh_public_key,
///         ssh_signature,
///         namespace: "doc-test@example.com".into(),
///         token: "d074a61990".to_string(),
///     };
///
///     ssh_verify(sig).await?;
///     Ok(())
/// }
/// ```
pub async fn ssh_verify(sig: SshSignature) -> Result<(), Box<dyn std::error::Error>> {
    let SshSignature {
        email,
        ssh_public_key,
        ssh_signature,
        namespace,
        token,
    } = sig;
    let dir = tempfile::tempdir()?;

    let mut allowed_signers_fp = NamedTempFile::new_in(dir.path())?;
    let mut signature_fp = NamedTempFile::new_in(dir.path())?;
    {
        let (tempfile, path) = allowed_signers_fp.into_parts();
        let mut file = File::from(tempfile);

        file.write_all(format!("{email} {ssh_public_key}").as_bytes())
            .await?;
        file.flush().await?;
        allowed_signers_fp = NamedTempFile::from_parts(file.into_std().await, path);
    }
    {
        let (tempfile, path) = signature_fp.into_parts();
        let mut file = File::from(tempfile);

        file.write_all(ssh_signature.trim().replace("\r\n", "\n").as_bytes())
            .await?;
        file.flush().await?;
        signature_fp = NamedTempFile::from_parts(file.into_std().await, path);
    }

    let mut cmd = Command::new("ssh-keygen");

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::piped());

    // Once you have your allowed signers file, verification works like this:
    //
    // ```shell
    // ssh-keygen -Y verify -f allowed_signers -I alice@example.com -n file -s file_to_verify.sig < file_to_verify
    // ```
    //
    // Here are the arguments you may need to change:
    //
    // - `allowed_signers` is the path to the allowed signers file.
    // - `alice@example.com` is the email address of the person who allegedly signed
    //   the file. This email address is looked up in the allowed signers file to
    //   get possible public keys.
    // - `file` is the "namespace", which must match the namespace used for signing
    //   as described above.
    // - `file_to_verify.sig` is the path to the signature file.
    // - `file_to_verify` is the path to the file to be verified. Note that this
    //   file is read from standard in. In the above command, the < shell operator
    //   is used to redirect standard in from this file.
    //
    // If the signature is valid, the command exits with status `0` and prints a
    // message like this:
    //
    // > Good "file" signature for alice@example.com with ED25519 key
    // > SHA256:ZGa8RztddW4kE2XKPPsP9ZYC7JnMObs6yZzyxg8xZSk
    //
    // Otherwise, the command exits with a non-zero status and prints an error
    // message.

    let mut child = cmd
        .arg("-Y")
        .arg("verify")
        .arg("-f")
        .arg(allowed_signers_fp.path())
        .arg("-I")
        .arg(&email)
        .arg("-n")
        .arg(namespace.as_ref())
        .arg("-s")
        .arg(signature_fp.path())
        .spawn()
        .expect("failed to spawn command");

    let mut stdin = child
        .stdin
        .take()
        .expect("child did not have a handle to stdin");

    stdin
        .write_all(token.as_bytes())
        .await
        .expect("could not write to stdin");

    drop(stdin);

    let op = child.wait_with_output().await?;

    if !op.status.success() {
        return Err(format!(
            "ssh-keygen exited with {}:\nstdout: {}\n\nstderr: {}",
            op.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&op.stdout),
            String::from_utf8_lossy(&op.stderr)
        )
        .into());
    }

    Ok(())
}

/// Run ssh signature validation.
///
/// ```no_run
/// use mailpot_web::{ssh_verify_in_memory, SshSignature};
///
/// async fn ssh_verify(
///     ssh_public_key: String,
///     ssh_signature: String,
/// ) -> std::result::Result<(), Box<dyn std::error::Error>> {
///     let sig = SshSignature {
///         email: "user@example.com".to_string(),
///         ssh_public_key,
///         ssh_signature,
///         namespace: "doc-test@example.com".into(),
///         token: "d074a61990".to_string(),
///     };
///
///     ssh_verify_in_memory(sig).await?;
///     Ok(())
/// }
/// ```
#[cfg(feature = "ssh-key")]
pub async fn ssh_verify_in_memory(sig: SshSignature) -> Result<(), Box<dyn std::error::Error>> {
    use ssh_key::{PublicKey, SshSig};

    let SshSignature {
        email: _,
        ref ssh_public_key,
        ref ssh_signature,
        ref namespace,
        ref token,
    } = sig;

    let public_key = ssh_public_key.parse::<PublicKey>().map_err(|err| {
        format!("Could not parse user's SSH public key. Is it valid? Reason given: {err}")
    })?;
    let signature = if ssh_signature.contains("\r\n") {
        ssh_signature.trim().replace("\r\n", "\n").parse::<SshSig>()
    } else {
        ssh_signature.parse::<SshSig>()
    }
    .map_err(|err| format!("Invalid SSH signature. Reason given: {err}"))?;

    if let Err(err) = public_key.verify(namespace, token.as_bytes(), &signature) {
        use ssh_key::Error;

        #[allow(clippy::wildcard_in_or_patterns)]
        return match err {
            Error::Io(err_kind) => {
                log::error!(
                    "ssh signature could not be verified because of internal error:\nSignature \
                     was {sig:#?}\nError was {err_kind}."
                );
                Err("SSH signature could not be verified because of internal error.".into())
            }
            Error::Crypto => Err("SSH signature is invalid.".into()),
            Error::AlgorithmUnknown
            | Error::AlgorithmUnsupported { .. }
            | Error::CertificateFieldInvalid(_)
            | Error::CertificateValidation
            | Error::Decrypted
            | Error::Ecdsa(_)
            | Error::Encoding(_)
            | Error::Encrypted
            | Error::FormatEncoding
            | Error::Namespace
            | Error::PublicKey
            | Error::Time
            | Error::TrailingData { .. }
            | Error::Version { .. }
            | _ => Err(format!("SSH signature could not be verified: Reason given: {err}").into()),
        };
    }

    Ok(())
}

pub async fn logout_handler(
    _: LogoutPath,
    mut auth: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Redirect {
    auth.logout().await;
    Redirect::to(&format!("{}/", state.root_url_prefix))
}

pub mod auth_request {
    use std::{marker::PhantomData, ops::RangeBounds};

    use axum::body::HttpBody;
    use dyn_clone::DynClone;
    use tower_http::auth::AuthorizeRequest;

    use super::*;

    trait RoleBounds<Role>: DynClone + Send + Sync {
        fn contains(&self, role: Option<Role>) -> bool;
    }

    impl<T, Role> RoleBounds<Role> for T
    where
        Role: PartialOrd + PartialEq,
        T: RangeBounds<Role> + Clone + Send + Sync,
    {
        fn contains(&self, role: Option<Role>) -> bool {
            role.as_ref()
                .map_or_else(|| role.is_none(), |role| RangeBounds::contains(self, role))
        }
    }

    /// Type that performs login authorization.
    ///
    /// See [`RequireAuthorizationLayer::login`] for more details.
    pub struct Login<UserId, User, ResBody, Role = ()> {
        login_url: Option<Arc<Cow<'static, str>>>,
        redirect_field_name: Option<Arc<Cow<'static, str>>>,
        role_bounds: Box<dyn RoleBounds<Role>>,
        _user_id_type: PhantomData<UserId>,
        _user_type: PhantomData<User>,
        _body_type: PhantomData<fn() -> ResBody>,
    }

    impl<UserId, User, ResBody, Role> Clone for Login<UserId, User, ResBody, Role> {
        fn clone(&self) -> Self {
            Self {
                login_url: self.login_url.clone(),
                redirect_field_name: self.redirect_field_name.clone(),
                role_bounds: dyn_clone::clone_box(&*self.role_bounds),
                _user_id_type: PhantomData,
                _user_type: PhantomData,
                _body_type: PhantomData,
            }
        }
    }

    impl<UserId, User, ReqBody, ResBody, Role> AuthorizeRequest<ReqBody>
        for Login<UserId, User, ResBody, Role>
    where
        Role: PartialOrd + PartialEq + Clone + Send + Sync + 'static,
        User: AuthUser<UserId, Role>,
        ResBody: HttpBody + Default,
    {
        type ResponseBody = ResBody;

        fn authorize(
            &mut self,
            request: &mut Request<ReqBody>,
        ) -> Result<(), Response<Self::ResponseBody>> {
            let user = request
                .extensions()
                .get::<Option<User>>()
                .expect("Auth extension missing. Is the auth layer installed?");

            match user {
                Some(user) if self.role_bounds.contains(user.get_role()) => {
                    let user = user.clone();
                    request.extensions_mut().insert(user);

                    Ok(())
                }

                _ => {
                    let unauthorized_response = if let Some(ref login_url) = self.login_url {
                        let url: Cow<'static, str> = self.redirect_field_name.as_ref().map_or_else(
                            || login_url.as_ref().clone(),
                            |next| {
                                format!(
                                    "{login_url}?{next}={}",
                                    percent_encoding::utf8_percent_encode(
                                        request.uri().path(),
                                        percent_encoding::CONTROLS
                                    )
                                )
                                .into()
                            },
                        );

                        Response::builder()
                            .status(http::StatusCode::TEMPORARY_REDIRECT)
                            .header(http::header::LOCATION, url.as_ref())
                            .body(Default::default())
                            .unwrap()
                    } else {
                        Response::builder()
                            .status(http::StatusCode::UNAUTHORIZED)
                            .body(Default::default())
                            .unwrap()
                    };

                    Err(unauthorized_response)
                }
            }
        }
    }

    /// A wrapper around [`tower_http::auth::RequireAuthorizationLayer`] which
    /// provides login authorization.
    pub struct RequireAuthorizationLayer<UserId, User, Role = ()>(UserId, User, Role);

    impl<UserId, User, Role> RequireAuthorizationLayer<UserId, User, Role>
    where
        Role: PartialOrd + PartialEq + Clone + Send + Sync + 'static,
        User: AuthUser<UserId, Role>,
    {
        /// Authorizes requests by requiring a logged in user, otherwise it
        /// rejects with [`http::StatusCode::UNAUTHORIZED`].
        pub fn login<ResBody>(
        ) -> tower_http::auth::RequireAuthorizationLayer<Login<UserId, User, ResBody, Role>>
        where
            ResBody: HttpBody + Default,
        {
            tower_http::auth::RequireAuthorizationLayer::custom(Login::<_, _, _, _> {
                login_url: None,
                redirect_field_name: None,
                role_bounds: Box::new(..),
                _user_id_type: PhantomData,
                _user_type: PhantomData,
                _body_type: PhantomData,
            })
        }

        /// Authorizes requests by requiring a logged in user to have a specific
        /// range of roles, otherwise it rejects with
        /// [`http::StatusCode::UNAUTHORIZED`].
        pub fn login_with_role<ResBody>(
            role_bounds: impl RangeBounds<Role> + Clone + Send + Sync + 'static,
        ) -> tower_http::auth::RequireAuthorizationLayer<Login<UserId, User, ResBody, Role>>
        where
            ResBody: HttpBody + Default,
        {
            tower_http::auth::RequireAuthorizationLayer::custom(Login::<_, _, _, _> {
                login_url: None,
                redirect_field_name: None,
                role_bounds: Box::new(role_bounds),
                _user_id_type: PhantomData,
                _user_type: PhantomData,
                _body_type: PhantomData,
            })
        }

        /// Authorizes requests by requiring a logged in user, otherwise it
        /// redirects to the provided login URL.
        ///
        /// If `redirect_field_name` is set to a value, the login page will
        /// receive the path it was redirected from in the URI query
        /// part. For example, attempting to visit a protected path
        /// `/protected` would redirect you to `/login?next=/protected` allowing
        /// you to know how to return the visitor to their requested
        /// page.
        pub fn login_or_redirect<ResBody>(
            login_url: Arc<Cow<'static, str>>,
            redirect_field_name: Option<Arc<Cow<'static, str>>>,
        ) -> tower_http::auth::RequireAuthorizationLayer<Login<UserId, User, ResBody, Role>>
        where
            ResBody: HttpBody + Default,
        {
            tower_http::auth::RequireAuthorizationLayer::custom(Login::<_, _, _, _> {
                login_url: Some(login_url),
                redirect_field_name,
                role_bounds: Box::new(..),
                _user_id_type: PhantomData,
                _user_type: PhantomData,
                _body_type: PhantomData,
            })
        }

        /// Authorizes requests by requiring a logged in user to have a specific
        /// range of roles, otherwise it redirects to the
        /// provided login URL.
        ///
        /// If `redirect_field_name` is set to a value, the login page will
        /// receive the path it was redirected from in the URI query
        /// part. For example, attempting to visit a protected path
        /// `/protected` would redirect you to `/login?next=/protected` allowing
        /// you to know how to return the visitor to their requested
        /// page.
        pub fn login_with_role_or_redirect<ResBody>(
            role_bounds: impl RangeBounds<Role> + Clone + Send + Sync + 'static,
            login_url: Arc<Cow<'static, str>>,
            redirect_field_name: Option<Arc<Cow<'static, str>>>,
        ) -> tower_http::auth::RequireAuthorizationLayer<Login<UserId, User, ResBody, Role>>
        where
            ResBody: HttpBody + Default,
        {
            tower_http::auth::RequireAuthorizationLayer::custom(Login::<_, _, _, _> {
                login_url: Some(login_url),
                redirect_field_name,
                role_bounds: Box::new(role_bounds),
                _user_id_type: PhantomData,
                _user_type: PhantomData,
                _body_type: PhantomData,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PKEY: &str = concat!(
        "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAACAQCzXp8nLJL8GPNw7S+Dqt0m3Dw/",
            "xFOAdwKXcekTFI9cLDEUII2rNPf0uUZTpv57OgU+",
            "QOEEIvWMjz+5KSWBX8qdP8OtV0QNvynlZkEKZN0cUqGKaNXo5a+PUDyiJ2rHroPe1aMo6mUBL9kLR6J2U1CYD/dLfL8ywXsAGmOL0bsK0GRPVBJAjpUNRjpGU/",
            "2FFIlU6s6GawdbDXEHDox/UoOVAKIlhKabaTrFBA0ACFLRX2/GCBmHqqt5d4ZZjefYzReLs/beOjafYImoyhHC428wZDcUjvLrpSJbIOE/",
            "gSPCWlRbcsxg4JGcKOtALUurE+ok+avy9M7eFjGhLGSlTKLdshIVQr/3W667M7bYfOT6xP/",
            "lyjxeWIUYyj7rjlqKJ9tzygek7QNxCtuqH5xsZAZqzQCN8wfrPAlwDykvWityKOw+Bt2DWjimITqyKgsBsOaA+",
            "eVCllFvooJxoYvAjODASjAUoOdgVzyBDpFnOhLFYiIIyL3F6NROS9i7z086paX7mrzcQzvLr4ckF9qT7DrI88ikISCR9bFR4vPq3aH",
            "zJdjDDpWxACa5b11NG8KdCJPe/L0kDw82Q00U13CpW9FI9sZjvk+",
            "lyw8bTFvVsIl6A0ueboFvrNvznAqHrtfWu75fXRh5sKj2TGk8rhm3vyNgrBSr5zAfFVM8LgqBxbAAYw=="
            );

    const ARMOR_SIG: &str = concat!(
        "-----BEGIN SSH SIGNATURE-----\n",
        "U1NIU0lHAAAAAQAAAhcAAAAHc3NoLXJzYQAAAAMBAAEAAAIBALNenycskvwY83DtL4Oq3S\n",
        "bcPD/EU4B3Apdx6RMUj1wsMRQgjas09/S5RlOm/ns6BT5A4QQi9YyPP7kpJYFfyp0/w61X\n",
        "RA2/KeVmQQpk3RxSoYpo1ejlr49QPKInaseug97VoyjqZQEv2QtHonZTUJgP90t8vzLBew\n",
        "AaY4vRuwrQZE9UEkCOlQ1GOkZT/YUUiVTqzoZrB1sNcQcOjH9Sg5UAoiWEpptpOsUEDQAI\n",
        "UtFfb8YIGYeqq3l3hlmN59jNF4uz9t46Np9giajKEcLjbzBkNxSO8uulIlsg4T+BI8JaVF\n",
        "tyzGDgkZwo60AtS6sT6iT5q/L0zt4WMaEsZKVMot2yEhVCv/dbrrsztth85PrE/+XKPF5Y\n",
        "hRjKPuuOWoon23PKB6TtA3EK26ofnGxkBmrNAI3zB+s8CXAPKS9aK3Io7D4G3YNaOKYhOr\n",
        "IqCwGw5oD55UKWUW+ignGhi8CM4MBKMBSg52BXPIEOkWc6EsViIgjIvcXo1E5L2LvPTzql\n",
        "pfuavNxDO8uvhyQX2pPsOsjzyKQhIJH1sVHi8+rdofMl2MMOlbEAJrlvXU0bwp0Ik978vS\n",
        "QPDzZDTRTXcKlb0Uj2xmO+T6XLDxtMW9WwiXoDS55ugW+s2/OcCoeu19a7vl9dGHmwqPZM\n",
        "aTyuGbe/I2CsFKvnMB8VUzwuCoHFsABjAAAAFGRvYy10ZXN0QGV4YW1wbGUuY29tAAAAAA\n",
        "AAAAZzaGE1MTIAAAIUAAAADHJzYS1zaGEyLTUxMgAAAgBxaMqIfeapKTrhQzggDssD+76s\n",
        "jZxv3XxzgsuAjlIdtw+/nyxU6skTnrGoam2shpmQvx0HuqSQ7HyS2USBK7T4LZNoE53zR/\n",
        "ZmHLGoyQAoexiHSEW9Lk53kyRNPhpXQedTvm8REHPGM3zw6WO6mAXVVxvebvawf81LTbBb\n",
        "p9ubNRcHgktVeywMO/sD6zWSyShq1gjVv1PdRBOjUgqkwjImL8dFKi1QUeoffCxyk3JhTO\n",
        "siTy79HZSz/kOvkvL1vQuqaP2R8lE9P1uaD19dGOMTPRod3u+QmpYX47ri5KM3Fmkfxdwq\n",
        "p8JVmfAA9nme7bmNS1hWgmF2Nbh9qjh1zOZvCimIpuNtz5eEl9K+1DxG6w5tX86wSGvBMO\n",
        "znx0k1gGfkiAULqgrkdul7mqMPRvPN9J6QlNJ7SLFChRhzlJIJc6tOvCs7qkVD43Zcb+I5\n",
        "Z+K4NiFf5jf8kVX/pjjeW/ucbrctJIkGsZ58OkHKi1EDRcq7NtCF6SKlcv8g3fMLd9wW6K\n",
        "aaed0TBDC+s+f6naNIGvWqfWCwDuK5xGyDTTmJGcrsMwWuT9K6uLk8cGdv7t5mOFuWi5jl\n",
        "E+IKZKVABMuWqSj96ErMIiBjtsAZfNSezpsK49wQztoSPhdwLhD6fHrSAyPCqN2xRkcsIb\n",
        "6PxWKC/OELf3gyEBRPouxsF7xSZQ==\n",
        "-----END SSH SIGNATURE-----\n"
    );

    fn create_sig() -> SshSignature {
        SshSignature {
            email: "user@example.com".to_string(),
            ssh_public_key: PKEY.to_string(),
            ssh_signature: ARMOR_SIG.to_string(),
            namespace: "doc-test@example.com".into(),
            token: "d074a61990".to_string(),
        }
    }

    #[tokio::test]
    async fn test_ssh_verify() {
        let mut sig = create_sig();
        ssh_verify(sig.clone()).await.unwrap();

        sig.ssh_signature = sig.ssh_signature.replace('J', "0");

        let err = ssh_verify(sig).await.unwrap_err();

        assert!(
            err.to_string().starts_with("ssh-keygen exited with"),
            "{}",
            err
        );
    }

    #[cfg(feature = "ssh-key")]
    #[tokio::test]
    async fn test_ssh_verify_in_memory() {
        let mut sig = create_sig();
        ssh_verify_in_memory(sig.clone()).await.unwrap();

        sig.ssh_signature = sig.ssh_signature.replace('J', "0");

        let err = ssh_verify_in_memory(sig.clone()).await.unwrap_err();

        assert_eq!(
            &err.to_string(),
            "Invalid SSH signature. Reason given: invalid label: 'ssh-}3a'",
            "{}",
            err
        );

        sig.ssh_public_key = sig.ssh_public_key.replace(' ', "0");

        let err = ssh_verify_in_memory(sig).await.unwrap_err();
        assert_eq!(
            &err.to_string(),
            "Could not parse user's SSH public key. Is it valid? Reason given: length invalid",
            "{}",
            err
        );

        let mut sig = create_sig();
        sig.token = sig.token.replace('d', "0");

        let err = ssh_verify_in_memory(sig).await.unwrap_err();
        assert_eq!(&err.to_string(), "SSH signature is invalid.", "{}", err);
    }
}
