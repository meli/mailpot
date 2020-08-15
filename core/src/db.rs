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
use melib::Envelope;
use models::changesets::*;
use rusqlite::Connection as DbConnection;
use rusqlite::OptionalExtension;
use std::path::PathBuf;

const DB_NAME: &str = "mpot.db";

pub struct Database {
    pub connection: DbConnection,
}

impl Database {
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

    pub fn db_path() -> Result<PathBuf> {
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
        let mut set_mode = false;
        if !db_path.exists() {
            info!("Creating {} database in {}", DB_NAME, db_path.display());
            set_mode = true;
        }
        let conn = DbConnection::open(&db_path.to_str().unwrap())?;
        if set_mode {
            use std::os::unix::fs::PermissionsExt;
            let file = std::fs::File::open(&db_path)?;
            let metadata = file.metadata()?;
            let mut permissions = metadata.permissions();

            permissions.set_mode(0o600); // Read/write for owner only.
            file.set_permissions(permissions)?;
        }

        Ok(Database { connection: conn })
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
        let address = env.from()[0].to_string();
        let message_id = env.message_id_display();
        let mut stmt = self.connection.prepare(
            "INSERT INTO post(list, address, message_id, message) VALUES(?, ?, ?, ?) RETURNING pk;",
        )?;
        let pk = stmt.query_row(
            rusqlite::params![&list_pk, &address, &message_id, &message],
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

    pub fn post(&self, env: Envelope, raw: &[u8], _dry_run: bool) -> Result<()> {
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
                    if let Err(err) = self.request(
                        list,
                        match subaddr.as_str() {
                            "subscribe" | "request" if env.subject().trim() == "subscribe" => {
                                ListRequest::Subscribe
                            }
                            "unsubscribe" | "request" if env.subject().trim() == "unsubscribe" => {
                                ListRequest::Unsubscribe
                            }
                            "request" => ListRequest::Other(env.subject().trim().to_string()),
                            _ => {
                                trace!(
                                    "unknown action = {} for addresses {:?} in list {}",
                                    subaddr,
                                    env.from(),
                                    list
                                );
                                ListRequest::Other(subaddr.trim().to_string())
                            }
                        },
                        &env,
                        raw,
                    ) {
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
            let post_pk = self.insert_post(list.pk, raw, &env)?;
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
                pk: post_pk,
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
            match configuration.send_mail {
                crate::config::SendMail::Smtp(ref smtp_conf) => {
                    let smtp_conf = smtp_conf.clone();
                    use melib::futures;
                    use melib::smol;
                    use melib::smtp::*;
                    let mut conn = smol::future::block_on(smol::spawn(
                        SmtpConnection::new_connection(smtp_conf.clone()),
                    ))?;
                    match action {
                        PostAction::Accept => {
                            for job in list_ctx.scheduled_jobs.iter() {
                                if let crate::mail::MailJob::Send {
                                    message_pk: _,
                                    recipients,
                                } = job
                                {
                                    futures::executor::block_on(conn.mail_transaction(
                                        &String::from_utf8_lossy(&bytes),
                                        Some(recipients),
                                    ))?;
                                }
                            }
                            /* - Save digest metadata in database */
                        }
                        PostAction::Reject { reason: _ } => {
                            /* - Notify submitter */
                            //futures::executor::block_on(conn.mail_transaction(&post.bytes, b)).unwrap();
                        }
                        PostAction::Defer { reason: _ } => {
                            /* - Notify submitter
                             * - Save in database */
                        }
                        PostAction::Hold => { /* - Save in database */ }
                    }
                }
                _ => {}
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
