use mailin_embedded::{Handler, Response, Server, SslConfig};
use mailpot::{melib, models::*, Configuration, Database, SendMail};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::thread;
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
        is8bit: bool,
        to: Vec<String>,
    },
    Data {
        from: String,
        is8bit: bool,
        to: Vec<String>,
        buf: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
struct MyHandler {
    mails: Vec<((IpAddr, String), Message)>,
}
use mailin_embedded::response::{INTERNAL_ERROR, OK};

impl Handler for MyHandler {
    fn helo(&mut self, ip: IpAddr, domain: &str) -> Response {
        eprintln!("helo ip {:?} domain {:?}", ip, domain);
        self.mails.push(((ip, domain.to_string()), Message::Helo));
        OK
    }

    fn mail(&mut self, ip: IpAddr, domain: &str, from: &str) -> Response {
        eprintln!("mail() ip {:?} domain {:?} from {:?}", ip, domain, from);
        if let Some((_, message)) = self
            .mails
            .iter_mut()
            .find(|((i, d), _)| (i, d.as_str()) == (&ip, domain))
        {
            std::dbg!(&message);
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
        eprintln!("rcpt() to {:?}", _to);
        if let Some((_, message)) = self.mails.last_mut() {
            std::dbg!(&message);
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
        eprintln!(
            "data_start() domain {:?} from {:?} is8bit {:?} to {:?}",
            _domain, _from, _is8bit, _to
        );
        if let Some(((_, d), ref mut message)) = self.mails.last_mut() {
            if d != _domain {
                return INTERNAL_ERROR;
            }
            std::dbg!(&message);
            if let Message::Rcpt { from, to } = message {
                *message = Message::DataStart {
                    from: from.to_string(),
                    is8bit: _is8bit,
                    to: _to.to_vec(),
                };
                return OK;
            }
        }
        INTERNAL_ERROR
    }

    fn data(&mut self, _buf: &[u8]) -> Result<(), std::io::Error> {
        if let Some(((_, _), ref mut message)) = self.mails.last_mut() {
            if let Message::DataStart { from, is8bit, to } = message {
                *message = Message::Data {
                    from: from.to_string(),
                    is8bit: *is8bit,
                    to: to.clone(),
                    buf: _buf.to_vec(),
                };
                return Ok(());
            } else if let Message::Data { buf, .. } = message {
                buf.extend(_buf.into_iter().copied());
                return Ok(());
            }
        }
        Ok(())
    }

    fn data_end(&mut self) -> Response {
        eprintln!("datae_nd() ");
        if let Some(((_, _), message)) = self.mails.pop() {
            if let Message::Data {
                from,
                is8bit,
                to,
                buf,
            } = message
            {
                match melib::Envelope::from_bytes(&buf, None) {
                    Ok(env) => {
                        std::dbg!(&env);
                        std::dbg!(env.other_headers());
                    }
                    Err(err) => {
                        eprintln!("envelope parse error {}", err);
                    }
                }
                return OK;
            }
        }
        INTERNAL_ERROR
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
    stderrlog::new()
        .quiet(false)
        .verbosity(15)
        .show_module_names(true)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();
    let tmp_dir = TempDir::new().unwrap();

    let _smtp_handle = thread::spawn(move || {
        let handler = MyHandler { mails: vec![] };
        let mut server = Server::new(handler);

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
    let mut config = Configuration::default();
    config.send_mail = SendMail::Smtp(get_smtp_conf());
    config.db_path = Some(db_path.clone());
    config.init_with().unwrap();

    assert_eq!(Database::db_path().unwrap(), db_path);

    let db = Database::open_or_create_db().unwrap();
    assert!(db.list_lists().unwrap().is_empty());
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
        .set_list_policy(
            foo_chat.pk(),
            PostPolicy {
                pk: 0,
                list: foo_chat.pk(),
                announce_only: false,
                subscriber_only: true,
                approval_needed: false,
            },
        )
        .unwrap();

    assert_eq!(post_policy.pk(), 1);

    let input_bytes = include_bytes!("./test_sample_longmessage.eml");
    match melib::Envelope::from_bytes(input_bytes, None) {
        Ok(envelope) => {
            eprintln!("envelope {:?}", &envelope);
            match db
                .post(&envelope, input_bytes, /* dry_run */ false)
                .unwrap_err()
                .kind()
            {
                mailpot::ErrorKind::PostRejected(_reason) => {}
                other => panic!("Got unexpected error: {}", other),
            }

            db.add_member(
                foo_chat.pk(),
                ListMembership {
                    pk: 0,
                    list: foo_chat.pk(),
                    address: "japoeunp@hotmail.com".into(),
                    name: Some("Jamaica Poe".into()),
                    digest: false,
                    hide_address: false,
                    receive_duplicates: true,
                    receive_own_posts: true,
                    receive_confirmation: true,
                    enabled: true,
                },
            )
            .unwrap();
            db.add_member(
                foo_chat.pk(),
                ListMembership {
                    pk: 0,
                    list: foo_chat.pk(),
                    address: "manos@example.com".into(),
                    name: Some("Manos Hands".into()),
                    digest: false,
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
}
