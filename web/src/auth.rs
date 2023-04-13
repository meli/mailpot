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
use std::borrow::Cow;
use tempfile::NamedTempFile;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use std::process::Stdio;

const TOKEN_KEY: &str = "ssh_challenge";
const EXPIRY_IN_SECS: i64 = 6 * 60;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum Role {
    User,
    Admin,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct User {
    pub id: i64,
    pub password_hash: String,
    pub role: Role,
    pub address: String,
}

impl AuthUser<i64, Role> for User {
    fn get_id(&self) -> i64 {
        self.id
    }

    fn get_password_hash(&self) -> SecretVec<u8> {
        SecretVec::new(self.password_hash.clone().into())
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
    mut session: WritableSession,
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
        return Redirect::to("/settings/").into_response();
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

    let (token, timestamp): (String, i64) = if let Some(tok) = prev_token {
        tok
    } else {
        use rand::distributions::Alphanumeric;
        use rand::{thread_rng, Rng};

        let mut rng = thread_rng();
        let chars: String = (0..7).map(|_| rng.sample(Alphanumeric) as char).collect();
        println!("Random chars: {}", chars);
        session.insert(TOKEN_KEY, (&chars, now)).unwrap();
        (chars, now)
    };
    let timeout_left = ((timestamp + EXPIRY_IN_SECS) - now) as f64 / 60.0;

    let root_url_prefix = &state.root_url_prefix;
    let crumbs = vec![
        Crumb {
            label: "Lists".into(),
            url: "/".into(),
        },
        Crumb {
            label: "Sign in".into(),
            url: "/login/".into(),
        },
    ];

    let context = minijinja::context! {
        namespace => &state.public_url,
        title => "mailing list archive",
        description => "",
        root_url_prefix => &root_url_prefix,
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

pub async fn ssh_signin_post(
    mut session: WritableSession,
    mut auth: AuthContext,
    Form(payload): Form<AuthFormPayload>,
    state: Arc<AppState>,
) -> Result<Redirect, ResponseError> {
    if auth.current_user.as_ref().is_some() {
        session.add_message(Message {
            message: "You are already logged in.".into(),
            level: Level::Info,
        })?;
        return Ok(Redirect::to("/settings/"));
    }

    let now: i64 = chrono::offset::Utc::now().timestamp();

    let (prev_token, _) =
        if let Some(tok @ (_, timestamp)) = session.get::<(String, i64)>(TOKEN_KEY) {
            if !(timestamp < now && now - timestamp < EXPIRY_IN_SECS) {
                session.add_message(Message {
                    message: "The token has expired. Please retry.".into(),
                    level: Level::Error,
                })?;
                return Ok(Redirect::to("/login/"));
            } else {
                tok
            }
        } else {
            session.add_message(Message {
                message: "The token has expired. Please retry.".into(),
                level: Level::Error,
            })?;
            return Ok(Redirect::to("/login/"));
        };

    drop(session);
    let db = Connection::open_db(state.conf.clone())?;
    let acc = match db
        .account_by_address(&payload.address)
        .with_status(StatusCode::BAD_REQUEST)?
    {
        Some(v) => v,
        None => {
            return Err(ResponseError::new(
                format!("Account for {} not found", payload.address),
                StatusCode::NOT_FOUND,
            ));
        }
    };
    let sig = SshSignature {
        email: payload.address.clone(),
        ssh_public_key: acc.password.clone(),
        ssh_signature: payload.password.clone(),
        namespace: "lists.mailpot.rs".into(),
        token: prev_token,
    };
    ssh_keygen(sig).await?;

    let user = User {
        id: acc.pk(),
        password_hash: payload.password,
        role: Role::User,
        address: payload.address,
    };
    state.insert_user(acc.pk(), user.clone()).await;
    auth.login(&user)
        .await
        .map_err(|err| ResponseError::new(err.to_string(), StatusCode::BAD_REQUEST))?;
    Ok(Redirect::to(&format!(
        "{}/settings/",
        state.root_url_prefix
    )))
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
/// use mpot_web::{ssh_keygen, SshSignature};
///
/// async fn key_gen(
///     ssh_public_key: String,
///     ssh_signature: String,
/// ) -> std::result::Result<(), Box<dyn std::error::Error>> {
///     let mut sig = SshSignature {
///         email: "user@example.com".to_string(),
///         ssh_public_key,
///         ssh_signature,
///         namespace: "doc-test@example.com".into(),
///         token: "d074a61990".to_string(),
///     };
///
///     ssh_keygen(sig.clone()).await?;
///     Ok(())
/// }
/// ```
pub async fn ssh_keygen(sig: SshSignature) -> Result<(), ResponseError> {
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
    // ssh-keygen -Y verify -f allowed_signers -I alice@example.com -n file -s file_to_verify.sig < file_to_verify
    // Here are the arguments you may need to change:
    //     allowed_signers is the path to the allowed signers file.
    //     alice@example.com is the email address of the person who allegedly signed the file. This email address is looked up in the allowed signers file to get possible public keys.
    //     file is the "namespace", which must match the namespace used for signing as described above.
    //     file_to_verify.sig is the path to the signature file.
    //     file_to_verify is the path to the file to be verified. Note that this file is read from standard in. In the above command, the < shell operator is used to redirect standard in from this file.
    // If the signature is valid, the command exits with status 0 and prints a message like this:
    // Good "file" signature for alice@example.com with ED25519 key SHA256:ZGa8RztddW4kE2XKPPsP9ZYC7JnMObs6yZzyxg8xZSk
    // Otherwise, the command exits with a non-zero status and prints an error message.

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
        return Err(ResponseError::new(
            format!(
                "ssh-keygen exited with {}:\nstdout: {}\n\nstderr: {}",
                op.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&op.stdout),
                String::from_utf8_lossy(&op.stderr)
            ),
            StatusCode::BAD_REQUEST,
        ));
    }

    Ok(())
}

pub async fn logout_handler(mut auth: AuthContext, State(state): State<Arc<AppState>>) -> Redirect {
    auth.logout().await;
    Redirect::to(&format!("{}/settings/", state.root_url_prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ssh_keygen() {
        const PKEY: &str = concat!("ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAACAQCzXp8nLJL8GPNw7S+Dqt0m3Dw/",
            "xFOAdwKXcekTFI9cLDEUII2rNPf0uUZTpv57OgU+",
            "QOEEIvWMjz+5KSWBX8qdP8OtV0QNvynlZkEKZN0cUqGKaNXo5a+PUDyiJ2rHroPe1aMo6mUBL9kLR6J2U1CYD/dLfL8ywXsAGmOL0bsK0GRPVBJAjpUNRjpGU/",
            "2FFIlU6s6GawdbDXEHDox/UoOVAKIlhKabaTrFBA0ACFLRX2/GCBmHqqt5d4ZZjefYzReLs/beOjafYImoyhHC428wZDcUjvLrpSJbIOE/",
            "gSPCWlRbcsxg4JGcKOtALUurE+ok+avy9M7eFjGhLGSlTKLdshIVQr/3W667M7bYfOT6xP/",
            "lyjxeWIUYyj7rjlqKJ9tzygek7QNxCtuqH5xsZAZqzQCN8wfrPAlwDykvWityKOw+Bt2DWjimITqyKgsBsOaA+",
            "eVCllFvooJxoYvAjODASjAUoOdgVzyBDpFnOhLFYiIIyL3F6NROS9i7z086paX7mrzcQzvLr4ckF9qT7DrI88ikISCR9bFR4vPq3aH",
            "zJdjDDpWxACa5b11NG8KdCJPe/L0kDw82Q00U13CpW9FI9sZjvk+",
            "lyw8bTFvVsIl6A0ueboFvrNvznAqHrtfWu75fXRh5sKj2TGk8rhm3vyNgrBSr5zAfFVM8LgqBxbAAYw==");

        const SIG: &str = concat!(
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
        const NAMESPACE: &str = "doc-test@example.com";

        let mut sig = SshSignature {
            email: "user@example.com".to_string(),
            ssh_public_key: PKEY.to_string(),
            ssh_signature: SIG.to_string(),
            namespace: "doc-test@example.com".into(),
            token: "d074a61990".to_string(),
        };

        ssh_keygen(sig.clone()).await.unwrap();

        sig.ssh_signature = sig.ssh_signature.replace("J", "0");

        let err = ssh_keygen(sig).await.unwrap_err();

        assert!(
            err.to_string().starts_with("ssh-keygen exited with"),
            "{}",
            err
        );
    }
}
