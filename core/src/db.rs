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
use crate::ErrorKind::*;
use melib::Envelope;
use models::changesets::*;
use rusqlite::Connection as DbConnection;
use rusqlite::OptionalExtension;
use std::convert::TryFrom;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const DB_NAME: &str = "mpot.db";

pub struct Database {
    pub connection: DbConnection,
}

mod error_queue;
pub use error_queue::*;

impl Database {
    pub fn db_path() -> Result<PathBuf> {
        let mut config_path = None;
        crate::config::CONFIG.with(|c| {
            config_path = c.borrow().db_path.clone();
        });
        if let Some(db_path) = config_path {
            return Ok(db_path);
        }
        let name = DB_NAME;
        let data_dir = xdg::BaseDirectories::with_prefix("mailpot")?;
        Ok(data_dir.place_data_file(name)?)
    }

    pub fn open_db(db_path: PathBuf) -> Result<Self> {
        if !db_path.exists() {
            return Err("Database doesn't exist".into());
        }
        Ok(Database {
            connection: DbConnection::open(&db_path.to_str().unwrap())?,
        })
    }

    pub fn open_or_create_db() -> Result<Self> {
        let db_path = Self::db_path()?;
        let mut create = false;
        if !db_path.exists() {
            info!("Creating {} database in {}", DB_NAME, db_path.display());
            create = true;
        }
        if create {
            use std::os::unix::fs::PermissionsExt;
            let mut child = Command::new("sqlite3")
                .arg(&db_path)
                .stdin(Stdio::piped())
                .spawn()?;
            let mut stdin = child.stdin.take().unwrap();
            std::thread::spawn(move || {
                stdin
                    .write_all(include_bytes!("./schema.sql"))
                    .expect("failed to write to stdin");
            });
            let output = child.wait_with_output()?;
            if !output.status.success() {
                return Err(format!("Could not initialize sqlite3 database at {}: sqlite3 returned exit code {} and stderr {}", db_path.display(), String::from_utf8_lossy(&output.stderr), output.status.code().unwrap_or_default()).into());
            }

            let file = std::fs::File::open(&db_path)?;
            let metadata = file.metadata()?;
            let mut permissions = metadata.permissions();

            permissions.set_mode(0o600); // Read/write for owner only.
            file.set_permissions(permissions)?;
        }

        let conn = DbConnection::open(&db_path.to_str().unwrap())?;

        Ok(Database { connection: conn })
    }

    pub fn load_archives(&mut self) -> Result<&mut Self> {
        let mut archives_path = None;
        crate::config::CONFIG.with(|c| {
            archives_path = c.borrow().archives_path.clone();
        });
        let archives_path = if let Some(archives_path) = archives_path {
            archives_path
        } else {
            return Ok(self);
        };
        let mut stmt = self.connection.prepare("ATTACH ? AS ?;")?;
        for archive in std::fs::read_dir(&archives_path)? {
            let archive = archive?;
            let path = archive.path();
            let name = path.file_name().unwrap_or_default();
            stmt.execute(rusqlite::params![
                path.to_str().unwrap(),
                name.to_str().unwrap()
            ])?;
        }
        drop(stmt);

        Ok(self)
    }

    pub fn list_lists(&self) -> Result<Vec<DbVal<MailingList>>> {
        let mut stmt = self.connection.prepare("SELECT * FROM mailing_lists;")?;
        let list_iter = stmt.query_map([], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                MailingList {
                    pk,
                    name: row.get("name")?,
                    id: row.get("id")?,
                    address: row.get("address")?,
                    description: row.get("description")?,
                    archive_url: row.get("archive_url")?,
                },
                pk,
            ))
        })?;

        let mut ret = vec![];
        for list in list_iter {
            let list = list?;
            ret.push(list);
        }
        Ok(ret)
    }

    pub fn get_list(&self, pk: i64) -> Result<Option<DbVal<MailingList>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM mailing_lists WHERE pk = ?;")?;
        let ret = stmt
            .query_row([&pk], |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    MailingList {
                        pk,
                        name: row.get("name")?,
                        id: row.get("id")?,
                        address: row.get("address")?,
                        description: row.get("description")?,
                        archive_url: row.get("archive_url")?,
                    },
                    pk,
                ))
            })
            .optional()?;

        Ok(ret)
    }

    pub fn get_list_by_id<S: AsRef<str>>(&self, id: S) -> Result<Option<DbVal<MailingList>>> {
        let id = id.as_ref();
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM mailing_lists WHERE id = ?;")?;
        let ret = stmt
            .query_row([&id], |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    MailingList {
                        pk,
                        name: row.get("name")?,
                        id: row.get("id")?,
                        address: row.get("address")?,
                        description: row.get("description")?,
                        archive_url: row.get("archive_url")?,
                    },
                    pk,
                ))
            })
            .optional()?;

        Ok(ret)
    }

    pub fn create_list(&self, new_val: MailingList) -> Result<DbVal<MailingList>> {
        let mut stmt = self
            .connection
            .prepare("INSERT INTO mailing_lists(name, id, address, description, archive_url) VALUES(?, ?, ?, ?, ?) RETURNING *;")?;
        let ret = stmt.query_row(
            rusqlite::params![
                &new_val.name,
                &new_val.id,
                &new_val.address,
                new_val.description.as_ref(),
                new_val.archive_url.as_ref(),
            ],
            |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    MailingList {
                        pk,
                        name: row.get("name")?,
                        id: row.get("id")?,
                        address: row.get("address")?,
                        description: row.get("description")?,
                        archive_url: row.get("archive_url")?,
                    },
                    pk,
                ))
            },
        )?;

        trace!("create_list {:?}.", &ret);
        Ok(ret)
    }

    pub fn remove_list_policy(&self, list_pk: i64, policy_pk: i64) -> Result<()> {
        let mut stmt = self
            .connection
            .prepare("DELETE FROM post_policy WHERE pk = ? AND list_pk = ?;")?;
        stmt.execute(rusqlite::params![&policy_pk, &list_pk,])?;

        trace!("remove_list_policy {} {}.", list_pk, policy_pk);
        Ok(())
    }

    pub fn set_list_policy(&self, list_pk: i64, policy: PostPolicy) -> Result<DbVal<PostPolicy>> {
        let mut stmt = self.connection.prepare("INSERT OR REPLACE INTO post_policy(list, announce_only, subscriber_only, approval_needed) VALUES (?, ?, ?, ?) RETURNING *;")?;
        let ret = stmt.query_row(
            rusqlite::params![
                &list_pk,
                &policy.announce_only,
                &policy.subscriber_only,
                &policy.approval_needed,
            ],
            |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    PostPolicy {
                        pk,
                        list: row.get("list")?,
                        announce_only: row.get("announce_only")?,
                        subscriber_only: row.get("subscriber_only")?,
                        approval_needed: row.get("approval_needed")?,
                    },
                    pk,
                ))
            },
        )?;

        trace!("set_list_policy {:?}.", &ret);
        Ok(ret)
    }

    pub fn list_posts(
        &self,
        list_pk: i64,
        _date_range: Option<(String, String)>,
    ) -> Result<Vec<DbVal<Post>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM post WHERE list = ?;")?;
        let iter = stmt.query_map(rusqlite::params![&list_pk,], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                Post {
                    pk,
                    list: row.get("list")?,
                    address: row.get("address")?,
                    message_id: row.get("message_id")?,
                    message: row.get("message")?,
                    timestamp: row.get("timestamp")?,
                    datetime: row.get("datetime")?,
                },
                pk,
            ))
        })?;
        let mut ret = vec![];
        for post in iter {
            let post = post?;
            ret.push(post);
        }

        trace!("list_posts {:?}.", &ret);
        Ok(ret)
    }

    pub fn update_list(&self, _change_set: MailingListChangeset) -> Result<()> {
        /*
        diesel::update(mailing_lists::table)
            .set(&set)
            .execute(&self.connection)?;
        */
        Ok(())
    }

    pub fn get_list_policy(&self, pk: i64) -> Result<Option<DbVal<PostPolicy>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM post_policy WHERE list = ?;")?;
        let ret = stmt
            .query_row([&pk], |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    PostPolicy {
                        pk,
                        list: row.get("list")?,
                        announce_only: row.get("announce_only")?,
                        subscriber_only: row.get("subscriber_only")?,
                        approval_needed: row.get("approval_needed")?,
                    },
                    pk,
                ))
            })
            .optional()?;

        Ok(ret)
    }

    pub fn get_list_owners(&self, pk: i64) -> Result<Vec<DbVal<ListOwner>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM list_owner WHERE list = ?;")?;
        let list_iter = stmt.query_map([&pk], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                ListOwner {
                    pk,
                    list: row.get("list")?,
                    address: row.get("address")?,
                    name: row.get("name")?,
                },
                pk,
            ))
        })?;

        let mut ret = vec![];
        for list in list_iter {
            let list = list?;
            ret.push(list);
        }
        Ok(ret)
    }

    pub fn remove_list_owner(&self, list_pk: i64, owner_pk: i64) -> Result<()> {
        self.connection
            .execute(
                "DELETE FROM list_owners WHERE list_pk = ? AND pk = ?;",
                rusqlite::params![&list_pk, &owner_pk],
            )
            .chain_err(|| NotFound("List owner"))?;
        Ok(())
    }

    pub fn add_list_owner(&self, list_pk: i64, list_owner: ListOwner) -> Result<DbVal<ListOwner>> {
        let mut stmt = self.connection.prepare(
            "INSERT OR REPLACE INTO list_owner(list, address, name) VALUES (?, ?, ?) RETURNING *;",
        )?;
        let ret = stmt.query_row(
            rusqlite::params![&list_pk, &list_owner.address, &list_owner.name,],
            |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    ListOwner {
                        pk,
                        list: row.get("list")?,
                        address: row.get("address")?,
                        name: row.get("name")?,
                    },
                    pk,
                ))
            },
        )?;

        trace!("add_list_owner {:?}.", &ret);
        Ok(ret)
    }

    pub fn list_members(&self, pk: i64) -> Result<Vec<DbVal<ListMembership>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT * FROM membership WHERE list = ?;")?;
        let list_iter = stmt.query_map([&pk], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                ListMembership {
                    pk: row.get("pk")?,
                    list: row.get("list")?,
                    address: row.get("address")?,
                    name: row.get("name")?,
                    digest: row.get("digest")?,
                    hide_address: row.get("hide_address")?,
                    receive_duplicates: row.get("receive_duplicates")?,
                    receive_own_posts: row.get("receive_own_posts")?,
                    receive_confirmation: row.get("receive_confirmation")?,
                    enabled: row.get("enabled")?,
                },
                pk,
            ))
        })?;

        let mut ret = vec![];
        for list in list_iter {
            let list = list?;
            ret.push(list);
        }
        Ok(ret)
    }

    pub fn add_member(
        &self,
        list_pk: i64,
        mut new_val: ListMembership,
    ) -> Result<DbVal<ListMembership>> {
        new_val.list = list_pk;
        let mut stmt = self
            .connection
            .prepare("INSERT INTO membership(list, address, name, enabled, digest, hide_address, receive_duplicates, receive_own_posts, receive_confirmation) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *;")?;
        let ret = stmt.query_row(
            rusqlite::params![
                &new_val.list,
                &new_val.address,
                &new_val.name,
                &new_val.enabled,
                &new_val.digest,
                &new_val.hide_address,
                &new_val.receive_duplicates,
                &new_val.receive_own_posts,
                &new_val.receive_confirmation
            ],
            |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    ListMembership {
                        pk,
                        list: row.get("list")?,
                        address: row.get("address")?,
                        name: row.get("name")?,
                        digest: row.get("digest")?,
                        hide_address: row.get("hide_address")?,
                        receive_duplicates: row.get("receive_duplicates")?,
                        receive_own_posts: row.get("receive_own_posts")?,
                        receive_confirmation: row.get("receive_confirmation")?,
                        enabled: row.get("enabled")?,
                    },
                    pk,
                ))
            },
        )?;

        trace!("add_member {:?}.", &ret);
        Ok(ret)
    }

    pub fn add_candidate_member(&self, list_pk: i64, mut new_val: ListMembership) -> Result<i64> {
        new_val.list = list_pk;
        let mut stmt = self
            .connection
            .prepare("INSERT INTO candidate_membership(list, address, name, accepted) VALUES(?, ?, ?, ?) RETURNING pk;")?;
        let ret = stmt.query_row(
            rusqlite::params![&new_val.list, &new_val.address, &new_val.name, &false,],
            |row| {
                let pk: i64 = row.get("pk")?;
                Ok(pk)
            },
        )?;

        trace!("add_candidate_member {:?}.", &ret);
        Ok(ret)
    }

    pub fn accept_candidate_member(&mut self, pk: i64) -> Result<DbVal<ListMembership>> {
        let tx = self.connection.transaction()?;
        let mut stmt = tx
            .prepare("INSERT INTO membership(list, address, name, enabled, digest, hide_address, receive_duplicates, receive_own_posts, receive_confirmation) FROM (SELECT list, address, name FROM candidate_membership WHERE pk = ?) RETURNING *;")?;
        let ret = stmt.query_row(rusqlite::params![&pk], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                ListMembership {
                    pk,
                    list: row.get("list")?,
                    address: row.get("address")?,
                    name: row.get("name")?,
                    digest: row.get("digest")?,
                    hide_address: row.get("hide_address")?,
                    receive_duplicates: row.get("receive_duplicates")?,
                    receive_own_posts: row.get("receive_own_posts")?,
                    receive_confirmation: row.get("receive_confirmation")?,
                    enabled: row.get("enabled")?,
                },
                pk,
            ))
        })?;
        drop(stmt);
        tx.execute(
            "UPDATE candidate_membership SET accepted = ? WHERE pk = ?;",
            [&pk],
        )?;
        tx.commit()?;

        trace!("accept_candidate_member {:?}.", &ret);
        Ok(ret)
    }

    pub fn remove_member(&self, list_pk: i64, address: &str) -> Result<()> {
        self.connection.execute(
            "DELETE FROM membership WHERE list_pk = ? AND address = ?;",
            rusqlite::params![&list_pk, &address],
        )?;

        Ok(())
    }

    pub fn update_member(&self, _change_set: ListMembershipChangeset) -> Result<()> {
        /*
        diesel::update(membership::table)
            .set(&set)
            .execute(&self.connection)?;
        */
        Ok(())
    }

    pub fn get_list_filters(
        &self,
        _list: &DbVal<MailingList>,
    ) -> Vec<Box<dyn crate::mail::message_filters::PostFilter>> {
        use crate::mail::message_filters::*;
        vec![
            Box::new(FixCRLF),
            Box::new(PostRightsCheck),
            Box::new(AddListHeaders),
            Box::new(FinalizeRecipients),
        ]
    }

    pub fn insert_post(&self, list_pk: i64, message: &[u8], env: &Envelope) -> Result<i64> {
        let address = env.from()[0].get_email();
        let message_id = env.message_id_display();
        let mut stmt = self.connection.prepare(
            "INSERT INTO post(list, address, message_id, message, datetime, timestamp) VALUES(?, ?, ?, ?, ?, ?) RETURNING pk;",
        )?;
        let pk = stmt.query_row(
            rusqlite::params![
                &list_pk,
                &address,
                &message_id,
                &message,
                &env.date,
                &env.timestamp
            ],
            |row| {
                let pk: i64 = row.get("pk")?;
                Ok(pk)
            },
        )?;

        trace!(
            "insert_post list_pk {}, from {:?} message_id {:?} post_pk {}.",
            list_pk,
            address,
            message_id,
            pk
        );
        Ok(pk)
    }

    pub fn post(&self, env: &Envelope, raw: &[u8], _dry_run: bool) -> Result<()> {
        trace!("Received envelope to post: {:#?}", &env);
        let tos = env.to().to_vec();
        if tos.is_empty() {
            return Err("Envelope To: field is empty!".into());
        }
        if env.from().is_empty() {
            return Err("Envelope From: field is empty!".into());
        }
        let mut lists = self.list_lists()?;
        for t in &tos {
            if let Some((addr, subaddr)) = t.subaddress("+") {
                lists.retain(|list| {
                    if !addr.contains_address(&list.list_address()) {
                        return true;
                    }
                    if let Err(err) = ListRequest::try_from((subaddr.as_str(), env))
                        .and_then(|req| self.request(list, req, env, raw))
                    {
                        info!("Processing request returned error: {}", err);
                    }
                    false
                });
            }
        }

        lists.retain(|list| {
            trace!(
                "Is post related to list {}? {}",
                &list,
                tos.iter().any(|a| a.contains_address(&list.list_address()))
            );

            tos.iter().any(|a| a.contains_address(&list.list_address()))
        });
        if lists.is_empty() {
            return Ok(());
        }

        let mut configuration = crate::config::Configuration::new();
        crate::config::CONFIG.with(|f| {
            configuration = f.borrow().clone();
        });
        trace!("Configuration is {:#?}", &configuration);
        use crate::mail::{ListContext, Post, PostAction};
        for mut list in lists {
            trace!("Examining list {}", list.list_id());
            let filters = self.get_list_filters(&list);
            let memberships = self.list_members(list.pk)?;
            trace!("List members {:#?}", &memberships);
            let mut list_ctx = ListContext {
                policy: self.get_list_policy(list.pk)?,
                list_owners: self.get_list_owners(list.pk)?,
                list: &mut list,
                memberships: &memberships,
                scheduled_jobs: vec![],
            };
            let mut post = Post {
                from: env.from()[0].clone(),
                bytes: raw.to_vec(),
                to: env.to().to_vec(),
                action: PostAction::Hold,
            };
            let result = filters
                .into_iter()
                .fold(Ok((&mut post, &mut list_ctx)), |p, f| {
                    p.and_then(|(p, c)| f.feed(p, c))
                });
            trace!("result {:#?}", result);

            let Post { bytes, action, .. } = post;
            let post_env = melib::Envelope::from_bytes(&bytes, None)?;
            match action {
                PostAction::Accept => {
                    let _post_pk = self.insert_post(list_ctx.list.pk, &bytes, &post_env)?;
                    for job in list_ctx.scheduled_jobs.iter() {
                        if let crate::mail::MailJob::Send { recipients } = job {
                            if !recipients.is_empty() {
                                trace!("recipients: {:?}", &recipients);

                                match &configuration.send_mail {
                                    crate::config::SendMail::Smtp(ref smtp_conf) => {
                                        let smtp_conf = smtp_conf.clone();
                                        use melib::futures;
                                        use melib::smol;
                                        use melib::smtp::*;
                                        let mut conn = smol::future::block_on(smol::spawn(
                                            SmtpConnection::new_connection(smtp_conf.clone()),
                                        ))?;
                                        futures::executor::block_on(conn.mail_transaction(
                                            &String::from_utf8_lossy(&bytes),
                                            Some(recipients),
                                        ))?;
                                    }
                                    _ => {}
                                }
                            } else {
                                trace!("list has no recipients");
                            }
                        }
                    }
                    /* - FIXME Save digest metadata in database */
                }
                PostAction::Reject { reason } => {
                    /* FIXME - Notify submitter */
                    trace!("PostAction::Reject {{ reason: {} }}", reason);
                    //futures::executor::block_on(conn.mail_transaction(&post.bytes, b)).unwrap();
                    return Err(PostRejected(reason).into());
                }
                PostAction::Defer { reason } => {
                    trace!("PostAction::Defer {{ reason: {} }}", reason);
                    /* - FIXME Notify submitter
                     * - FIXME Save in database */
                }
                PostAction::Hold => {
                    trace!("PostAction::Hold");
                    /* FIXME - Save in database */
                }
            }
        }

        Ok(())
    }

    pub fn request(
        &self,
        list: &DbVal<MailingList>,
        request: ListRequest,
        env: &Envelope,
        _raw: &[u8],
    ) -> Result<()> {
        match request {
            ListRequest::Subscribe => {
                trace!(
                    "subscribe action for addresses {:?} in list {}",
                    env.from(),
                    list
                );

                let list_policy = self.get_list_policy(list.pk)?;
                let approval_needed = list_policy
                    .as_ref()
                    .map(|p| p.approval_needed)
                    .unwrap_or(false);
                for f in env.from() {
                    let membership = ListMembership {
                        pk: 0,
                        list: list.pk,
                        address: f.get_email(),
                        name: f.get_display_name(),
                        digest: false,
                        hide_address: false,
                        receive_duplicates: true,
                        receive_own_posts: false,
                        receive_confirmation: true,
                        enabled: !approval_needed,
                    };
                    if approval_needed {
                        match self.add_candidate_member(list.pk, membership) {
                            Ok(_) => {}
                            Err(_err) => {}
                        }
                        //FIXME: send notification to list-owner
                    } else if let Err(_err) = self.add_member(list.pk, membership) {
                        //FIXME: send failure notice to f
                    } else {
                        //FIXME: send success notice
                    }
                }
            }
            ListRequest::Unsubscribe => {
                trace!(
                    "unsubscribe action for addresses {:?} in list {}",
                    env.from(),
                    list
                );
                for f in env.from() {
                    if let Err(_err) = self.remove_member(list.pk, &f.get_email()) {
                        //FIXME: send failure notice to f
                    } else {
                        //FIXME: send success notice to f
                    }
                }
            }
            ListRequest::Other(ref req) if req == "owner" => {
                trace!(
                    "list-owner mail action for addresses {:?} in list {}",
                    env.from(),
                    list
                );
                //FIXME: mail to list-owner
            }
            ListRequest::RetrieveMessages(ref message_ids) => {
                trace!(
                    "retrieve messages {:?} action for addresses {:?} in list {}",
                    message_ids,
                    env.from(),
                    list
                );
                //FIXME
            }
            ListRequest::RetrieveArchive(ref from, ref to) => {
                trace!(
                    "retrieve archie action from {:?} to {:?} for addresses {:?} in list {}",
                    from,
                    to,
                    env.from(),
                    list
                );
                //FIXME
            }
            ListRequest::SetDigest(ref toggle) => {
                trace!(
                    "set digest action with value {} for addresses {:?} in list {}",
                    toggle,
                    env.from(),
                    list
                );
            }
            ListRequest::Other(ref req) => {
                trace!(
                    "unknown request action {} for addresses {:?} in list {}",
                    req,
                    env.from(),
                    list
                );
            }
        }
        Ok(())
    }
}
