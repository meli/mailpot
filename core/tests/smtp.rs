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

mod utils;

use std::net::IpAddr; //, Ipv4Addr, Ipv6Addr};
use std::{
    sync::{Arc, Mutex},
    thread,
};

use log::{trace, warn};
use mailin_embedded::{Handler, Response, Server, SslConfig};
use mailpot::{melib, models::*, Configuration, Connection, Queue, SendMail};
use melib::smol;
use tempfile::TempDir;

const ADDRESS: &str = "127.0.0.1:8825";
#[derive(Debug, Clone)]
enum Message {
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
struct MyHandler {
    mails: Arc<Mutex<Vec<((IpAddr, String), Message)>>>,
    stored: Arc<Mutex<Vec<(String, melib::Envelope)>>>,
}
use mailin_embedded::response::{INTERNAL_ERROR, OK};

impl Handler for MyHandler {
    fn helo(&mut self, ip: IpAddr, domain: &str) -> Response {
        // eprintln!("helo ip {:?} domain {:?}", ip, domain);
        self.mails
            .lock()
            .unwrap()
            .push(((ip, domain.to_string()), Message::Helo));
        OK
    }

    fn mail(&mut self, ip: IpAddr, domain: &str, from: &str) -> Response {
        // eprintln!("mail() ip {:?} domain {:?} from {:?}", ip, domain, from);
        if let Some((_, message)) = self
            .mails
            .lock()
            .unwrap()
            .iter_mut()
            .find(|((i, d), _)| (i, d.as_str()) == (&ip, domain))
        {
            if let Message::Helo = message {
                *message = Message::Mail {
                    from: from.to_string(),
                };
                return OK;
            }
        }
        INTERNAL_ERROR
    }

    fn rcpt(&mut self, _to: &str) -> Response {
        // eprintln!("rcpt() to {:?}", _to);
        if let Some((_, message)) = self.mails.lock().unwrap().last_mut() {
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
        if let Some(((_, d), ref mut message)) = self.mails.lock().unwrap().last_mut() {
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
        if let Some(((_, _), ref mut message)) = self.mails.lock().unwrap().last_mut() {
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
        let last = self.mails.lock().unwrap().pop();
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
            self.mails
                .lock()
                .unwrap()
                .push(((ip, domain), Message::Helo));
            return OK;
        }
        panic!("last self.mails item was not Message::Data: {last:?}"); //INTERNAL_ERROR
    }
}

fn get_smtp_conf() -> melib::smtp::SmtpServerConf {
    use melib::smtp::*;
    SmtpServerConf {
        hostname: "127.0.0.1".into(),
        port: 8825,
        envelope_from: "foo-chat@example.com".into(),
        auth: SmtpAuth::None,
        security: SmtpSecurity::None,
        extensions: Default::default(),
    }
}

#[test]
fn test_smtp() {
    utils::init_stderr_logging();

    let tmp_dir = TempDir::new().unwrap();

    let handler = MyHandler {
        mails: Arc::new(Mutex::new(vec![])),
        stored: Arc::new(Mutex::new(vec![])),
    };
    let handler2 = handler.clone();
    let _smtp_handle = thread::spawn(move || {
        let mut server = Server::new(handler2);

        server
            .with_name("example.com")
            .with_ssl(SslConfig::None)
            .unwrap()
            .with_addr(ADDRESS)
            .unwrap();
        eprintln!("Running smtp server at {}", ADDRESS);
        server.serve().expect("Could not run server");
    });

    let db_path = tmp_dir.path().join("mpot.db");
    let config = Configuration {
        send_mail: SendMail::Smtp(get_smtp_conf()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
    };

    let mut db = Connection::open_or_create_db(config).unwrap().trusted();
    assert!(db.lists().unwrap().is_empty());
    let foo_chat = db
        .create_list(MailingList {
            pk: 0,
            name: "foobar chat".into(),
            id: "foo-chat".into(),
            address: "foo-chat@example.com".into(),
            description: None,
            archive_url: None,
        })
        .unwrap();

    assert_eq!(foo_chat.pk(), 1);
    let post_policy = db
        .set_list_post_policy(PostPolicy {
            pk: 0,
            list: foo_chat.pk(),
            announce_only: false,
            subscription_only: true,
            approval_needed: false,
            open: false,
            custom: false,
        })
        .unwrap();

    assert_eq!(post_policy.pk(), 1);

    let input_bytes = include_bytes!("./test_sample_longmessage.eml");
    match melib::Envelope::from_bytes(input_bytes, None) {
        Ok(envelope) => {
            // eprintln!("envelope {:?}", &envelope);
            match db
                .post(&envelope, input_bytes, /* dry_run */ false)
                .unwrap_err()
                .kind()
            {
                mailpot::ErrorKind::PostRejected(reason) => {
                    trace!("Non-subscription post succesfully rejected: '{reason}'");
                }
                other => panic!("Got unexpected error: {}", other),
            }

            db.add_subscription(
                foo_chat.pk(),
                ListSubscription {
                    pk: 0,
                    list: foo_chat.pk(),
                    address: "japoeunp@example.com".into(),
                    name: Some("Jamaica Poe".into()),
                    account: None,
                    digest: false,
                    verified: true,
                    hide_address: false,
                    receive_duplicates: true,
                    receive_own_posts: true,
                    receive_confirmation: true,
                    enabled: true,
                },
            )
            .unwrap();
            db.add_subscription(
                foo_chat.pk(),
                ListSubscription {
                    pk: 0,
                    list: foo_chat.pk(),
                    address: "manos@example.com".into(),
                    name: Some("Manos Hands".into()),
                    account: None,
                    digest: false,
                    verified: true,
                    hide_address: false,
                    receive_duplicates: true,
                    receive_own_posts: true,
                    receive_confirmation: true,
                    enabled: true,
                },
            )
            .unwrap();
            db.post(&envelope, input_bytes, /* dry_run */ false)
                .unwrap();
        }
        Err(err) => {
            panic!("Could not parse message: {}", err);
        }
    }
    let messages = db.delete_from_queue(Queue::Out, vec![]).unwrap();
    eprintln!("Queue out has {} messages.", messages.len());
    let conn_future = db.new_smtp_connection().unwrap();
    smol::future::block_on(smol::spawn(async move {
        let mut conn = conn_future.await.unwrap();
        for msg in messages {
            Connection::submit(&mut conn, &msg, /* dry_run */ false)
                .await
                .unwrap();
        }
    }));
    let stored = handler.stored.lock().unwrap();
    assert_eq!(stored.len(), 3);
    assert_eq!(&stored[0].0, "japoeunp@example.com");
    assert_eq!(
        &stored[0].1.subject(),
        "Your post to foo-chat was rejected."
    );
    assert_eq!(
        &stored[1].1.subject(),
        "[foo-chat] thankful that I had the chance to written report, that I could learn and let \
         alone the chance $4454.32"
    );
    assert_eq!(
        &stored[2].1.subject(),
        "[foo-chat] thankful that I had the chance to written report, that I could learn and let \
         alone the chance $4454.32"
    );
}

#[test]
fn test_smtp_mailcrab() {
    use std::env;
    utils::init_stderr_logging();

    fn get_smtp_conf() -> melib::smtp::SmtpServerConf {
        use melib::smtp::*;
        SmtpServerConf {
            hostname: "127.0.0.1".into(),
            port: 1025,
            envelope_from: "foo-chat@example.com".into(),
            auth: SmtpAuth::None,
            security: SmtpSecurity::None,
            extensions: Default::default(),
        }
    }

    let Ok(mailcrab_ip) = env::var("MAILCRAB_IP") else {
        warn!("MAILCRAB_IP env var not set, is mailcrab server running?");
        return;
    };
    let mailcrab_port = env::var("MAILCRAB_PORT").unwrap_or("1080".to_string());
    let api_uri = format!("http://{mailcrab_ip}:{mailcrab_port}/api/messages");

    let tmp_dir = TempDir::new().unwrap();

    let db_path = tmp_dir.path().join("mpot.db");
    let config = Configuration {
        send_mail: SendMail::Smtp(get_smtp_conf()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
    };

    let mut db = Connection::open_or_create_db(config).unwrap().trusted();
    assert!(db.lists().unwrap().is_empty());
    let foo_chat = db
        .create_list(MailingList {
            pk: 0,
            name: "foobar chat".into(),
            id: "foo-chat".into(),
            address: "foo-chat@example.com".into(),
            description: None,
            archive_url: None,
        })
        .unwrap();

    assert_eq!(foo_chat.pk(), 1);
    let post_policy = db
        .set_list_post_policy(PostPolicy {
            pk: 0,
            list: foo_chat.pk(),
            announce_only: false,
            subscription_only: true,
            approval_needed: false,
            open: false,
            custom: false,
        })
        .unwrap();

    assert_eq!(post_policy.pk(), 1);

    let input_bytes = include_bytes!("./test_sample_longmessage.eml");
    match melib::Envelope::from_bytes(input_bytes, None) {
        Ok(envelope) => {
            match db
                .post(&envelope, input_bytes, /* dry_run */ false)
                .unwrap_err()
                .kind()
            {
                mailpot::ErrorKind::PostRejected(reason) => {
                    trace!("Non-subscription post succesfully rejected: '{reason}'");
                }
                other => panic!("Got unexpected error: {}", other),
            }
            db.add_subscription(
                foo_chat.pk(),
                ListSubscription {
                    pk: 0,
                    list: foo_chat.pk(),
                    address: "japoeunp@example.com".into(),
                    name: Some("Jamaica Poe".into()),
                    account: None,
                    digest: false,
                    verified: true,
                    hide_address: false,
                    receive_duplicates: true,
                    receive_own_posts: true,
                    receive_confirmation: true,
                    enabled: true,
                },
            )
            .unwrap();
            db.add_subscription(
                foo_chat.pk(),
                ListSubscription {
                    pk: 0,
                    list: foo_chat.pk(),
                    address: "manos@example.com".into(),
                    name: Some("Manos Hands".into()),
                    account: None,
                    digest: false,
                    verified: true,
                    hide_address: false,
                    receive_duplicates: true,
                    receive_own_posts: true,
                    receive_confirmation: true,
                    enabled: true,
                },
            )
            .unwrap();
            db.post(&envelope, input_bytes, /* dry_run */ false)
                .unwrap();
        }
        Err(err) => {
            panic!("Could not parse message: {}", err);
        }
    }
    let mails: String = reqwest::blocking::get(api_uri).unwrap().text().unwrap();
    trace!("mails: {}", mails);
}
