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

#![allow(clippy::new_without_default)]

use std::net::IpAddr; //, Ipv4Addr, Ipv6Addr};
use std::{
    borrow::Cow,
    net::ToSocketAddrs,
    sync::{Arc, Mutex, Once},
    thread,
};

pub use assert_cmd;
pub use log::{trace, warn};
use mailin_embedded::{
    response::{INTERNAL_ERROR, OK},
    Handler, Response, Server,
};
pub use mailpot::{
    melib::{self, smol, smtp::SmtpServerConf},
    models::{changesets::ListSubscriptionChangeset, *},
    Configuration, Connection, Queue, SendMail,
};
pub use predicates;
pub use tempfile::{self, TempDir};

static INIT_STDERR_LOGGING: Once = Once::new();

pub fn init_stderr_logging() {
    INIT_STDERR_LOGGING.call_once(|| {
        stderrlog::new()
            .quiet(false)
            .verbosity(15)
            .show_module_names(true)
            .timestamp(stderrlog::Timestamp::Millisecond)
            .init()
            .unwrap();
    });
}
pub const ADDRESS: &str = "127.0.0.1:8825";

#[derive(Debug, Clone)]
pub enum Message {
    Helo,
    Mail {
        from: String,
    },
    Rcpt {
        from: String,
        to: Vec<String>,
    },
    DataStart {
        from: String,
        to: Vec<String>,
    },
    Data {
        #[allow(dead_code)]
        from: String,
        to: Vec<String>,
        buf: Vec<u8>,
    },
}

#[allow(clippy::type_complexity)]
#[derive(Debug, Clone)]
pub struct TestSmtpHandler {
    address: Cow<'static, str>,
    ssl: SslConfig,
    envelope_from: Cow<'static, str>,
    auth: melib::smtp::SmtpAuth,
    pub messages: Arc<Mutex<Vec<((IpAddr, String), Message)>>>,
    pub stored: Arc<Mutex<Vec<(String, melib::Envelope)>>>,
}

impl Handler for TestSmtpHandler {
    fn helo(&mut self, ip: IpAddr, domain: &str) -> Response {
        //eprintln!("helo ip {:?} domain {:?}", ip, domain);
        self.messages
            .lock()
            .unwrap()
            .push(((ip, domain.to_string()), Message::Helo));
        OK
    }

    fn mail(&mut self, ip: IpAddr, domain: &str, from: &str) -> Response {
        //eprintln!("mail() ip {:?} domain {:?} from {:?}", ip, domain, from);
        if let Some((_, message)) = self
            .messages
            .lock()
            .unwrap()
            .iter_mut()
            .rev()
            .find(|((i, d), _)| (i, d.as_str()) == (&ip, domain))
        {
            if let Message::Helo = &message {
                *message = Message::Mail {
                    from: from.to_string(),
                };
                return OK;
            }
        }
        INTERNAL_ERROR
    }

    fn rcpt(&mut self, _to: &str) -> Response {
        //eprintln!("rcpt() to {:?}", _to);
        if let Some((_, message)) = self.messages.lock().unwrap().last_mut() {
            if let Message::Mail { from } = message {
                *message = Message::Rcpt {
                    from: from.clone(),
                    to: vec![_to.to_string()],
                };
                return OK;
            } else if let Message::Rcpt { to, .. } = message {
                to.push(_to.to_string());
                return OK;
            }
        }
        INTERNAL_ERROR
    }

    fn data_start(
        &mut self,
        _domain: &str,
        _from: &str,
        _is8bit: bool,
        _to: &[String],
    ) -> Response {
        // eprintln!( "data_start() domain {:?} from {:?} is8bit {:?} to {:?}", _domain,
        // _from, _is8bit, _to);
        if let Some(((_, d), ref mut message)) = self.messages.lock().unwrap().last_mut() {
            if d != _domain {
                return INTERNAL_ERROR;
            }
            if let Message::Rcpt { from, to } = message {
                *message = Message::DataStart {
                    from: from.to_string(),
                    to: to.to_vec(),
                };
                return OK;
            }
        }
        INTERNAL_ERROR
    }

    fn data(&mut self, _buf: &[u8]) -> Result<(), std::io::Error> {
        if let Some(((_, _), ref mut message)) = self.messages.lock().unwrap().last_mut() {
            if let Message::DataStart { from, to } = message {
                *message = Message::Data {
                    from: from.to_string(),
                    to: to.clone(),
                    buf: _buf.to_vec(),
                };
                return Ok(());
            } else if let Message::Data { buf, .. } = message {
                buf.extend(_buf.iter());
                return Ok(());
            }
        }
        Ok(())
    }

    fn data_end(&mut self) -> Response {
        let last = self.messages.lock().unwrap().pop();
        if let Some(((ip, domain), Message::Data { from: _, to, buf })) = last {
            for to in to {
                match melib::Envelope::from_bytes(&buf, None) {
                    Ok(env) => {
                        self.stored.lock().unwrap().push((to.clone(), env));
                    }
                    Err(err) => {
                        panic!("envelope parse error {}", err);
                    }
                }
            }
            self.messages
                .lock()
                .unwrap()
                .push(((ip, domain), Message::Helo));
            return OK;
        }
        panic!("last self.messages item was not Message::Data: {last:?}"); //INTERNAL_ERROR
    }
}

impl TestSmtpHandler {
    #[inline]
    pub fn smtp_conf(&self) -> melib::smtp::SmtpServerConf {
        use melib::smtp::*;
        let sockaddr = self
            .address
            .as_ref()
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap();
        let ip = sockaddr.ip();
        let port = sockaddr.port();

        SmtpServerConf {
            hostname: ip.to_string(),
            port,
            envelope_from: self.envelope_from.to_string(),
            auth: self.auth.clone(),
            security: SmtpSecurity::None,
            extensions: Default::default(),
        }
    }
}

impl TestSmtpHandler {
    pub fn builder() -> TestSmtpHandlerBuilder {
        TestSmtpHandlerBuilder::new()
    }
}

pub struct TestSmtpHandlerBuilder {
    address: Cow<'static, str>,
    ssl: SslConfig,
    auth: melib::smtp::SmtpAuth,
    envelope_from: Cow<'static, str>,
}

impl TestSmtpHandlerBuilder {
    pub fn new() -> Self {
        Self {
            address: ADDRESS.into(),
            ssl: SslConfig::None,
            auth: melib::smtp::SmtpAuth::None,
            envelope_from: "foo-chat@example.com".into(),
        }
    }

    pub fn address(self, address: impl Into<Cow<'static, str>>) -> Self {
        Self {
            address: address.into(),
            ..self
        }
    }

    pub fn ssl(self, ssl: SslConfig) -> Self {
        Self { ssl, ..self }
    }

    pub fn build(self) -> TestSmtpHandler {
        let Self {
            address,
            ssl,
            auth,
            envelope_from,
        } = self;
        let handler = TestSmtpHandler {
            address,
            ssl,
            auth,
            envelope_from,
            messages: Arc::new(Mutex::new(vec![])),
            stored: Arc::new(Mutex::new(vec![])),
        };
        crate::init_stderr_logging();
        let handler2 = handler.clone();
        let _smtp_handle = thread::spawn(move || {
            let address = handler2.address.clone();
            let ssl = handler2.ssl.clone();

            let mut server = Server::new(handler2.clone());
            let sockaddr = address.as_ref().to_socket_addrs().unwrap().next().unwrap();
            let ip = sockaddr.ip();
            let port = sockaddr.port();
            let addr = std::net::SocketAddr::new(ip, port);
            eprintln!("Running smtp server at {}", addr);
            server
                .with_name("example.com")
                .with_ssl((&ssl).into())
                .unwrap()
                .with_addr(addr)
                .unwrap();
            server.serve().expect("Could not run server");
        });
        handler
    }
}

/// Mirror struct for [`mailin_embedded::SslConfig`] because it does not
/// implement Debug or Clone.
#[derive(Clone, Debug)]
pub enum SslConfig {
    /// Do not support STARTTLS
    None,
    /// Use a self-signed certificate for STARTTLS
    SelfSigned {
        /// Certificate path
        cert_path: String,
        /// Path to key file
        key_path: String,
    },
    /// Use a certificate from an authority
    Trusted {
        /// Certificate path
        cert_path: String,
        /// Key file path
        key_path: String,
        /// Path to CA bundle
        chain_path: String,
    },
}

impl From<&SslConfig> for mailin_embedded::SslConfig {
    fn from(val: &SslConfig) -> Self {
        match val {
            SslConfig::None => Self::None,
            SslConfig::SelfSigned {
                ref cert_path,
                ref key_path,
            } => Self::SelfSigned {
                cert_path: cert_path.to_string(),
                key_path: key_path.to_string(),
            },
            SslConfig::Trusted {
                ref cert_path,
                ref key_path,
                ref chain_path,
            } => Self::Trusted {
                cert_path: cert_path.to_string(),
                key_path: key_path.to_string(),
                chain_path: chain_path.to_string(),
            },
        }
    }
}
