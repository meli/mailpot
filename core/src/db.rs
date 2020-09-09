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
use diesel::{prelude::*, Connection};
use melib::Envelope;
use std::path::PathBuf;

const DB_NAME: &str = "mpot.db";

pub struct Database {
    connection: SqliteConnection,
}

impl Database {
    pub fn list_lists(&self) -> Result<Vec<MailingList>> {
        use schema::mailing_lists;

        let ret = mailing_lists::table.load(&self.connection)?;
        Ok(ret)
    }

    pub fn get_list(&self, pk: i32) -> Result<Option<MailingList>> {
        use schema::mailing_lists;

        let ret = mailing_lists::table
            .filter(mailing_lists::pk.eq(pk))
            .first(&self.connection)
            .optional()?;
        Ok(ret)
    }

    pub fn get_list_policy(&self, pk: i32) -> Result<Option<PostPolicy>> {
        use schema::post_policy;

        let ret = post_policy::table
            .filter(post_policy::list.eq(pk))
            .first(&self.connection)
            .optional()?;
        Ok(ret)
    }

    pub fn get_list_owners(&self, pk: i32) -> Result<Vec<ListOwner>> {
        use schema::list_owner;

        let ret = list_owner::table
            .filter(list_owner::list.eq(pk))
            .load(&self.connection)?;
        Ok(ret)
    }

    pub fn list_members(&self, pk: i32) -> Result<Vec<ListMembership>> {
        use schema::membership;

        let ret = membership::table
            .filter(membership::list.eq(pk))
            .load(&self.connection)?;
        Ok(ret)
    }

    pub fn add_member(&self, list_pk: i32, mut new_val: ListMembership) -> Result<()> {
        use schema::membership;
        new_val.list = list_pk;

        diesel::insert_into(membership::table)
            .values(&new_val)
            .execute(&self.connection)?;
        Ok(())
    }

    pub fn remove_member(&self, list_pk: i32, address: &str) -> Result<()> {
        use schema::membership;
        diesel::delete(
            membership::table
                .filter(membership::columns::list.eq(list_pk))
                .filter(membership::columns::address.eq(address)),
        )
        .execute(&self.connection)?;
        Ok(())
    }

    pub fn create_list(&self, new_val: MailingList) -> Result<()> {
        use schema::mailing_lists;

        diesel::insert_into(mailing_lists::table)
            .values(&new_val)
            .execute(&self.connection)?;
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
            connection: SqliteConnection::establish(&db_path.to_str().unwrap())?,
        })
    }

    pub fn open_or_create_db() -> Result<Self> {
        let db_path = Self::db_path()?;
        let mut set_mode = false;
        if !db_path.exists() {
            info!("Creating {} database in {}", DB_NAME, db_path.display());
            set_mode = true;
        }
        let conn = SqliteConnection::establish(&db_path.to_str().unwrap())?;
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

    pub fn get_list_filters(&self, _list: &MailingList) -> Vec<Box<dyn crate::post::PostFilter>> {
        use crate::post::*;
        vec![
            Box::new(FixCRLF),
            Box::new(PostRightsCheck),
            Box::new(AddListHeaders),
            Box::new(FinalizeRecipients),
        ]
    }

    pub fn post(&self, env: Envelope, raw: &[u8]) -> Result<()> {
        trace!("Received envelope to post: {:#?}", &env);
        let mut lists = self.list_lists()?;
        let tos = env.to().iter().cloned().collect::<Vec<_>>();
        if tos.is_empty() {
            return Err("Envelope To: field is empty!".into());
        }
        if env.from().is_empty() {
            return Err("Envelope From: field is empty!".into());
        }
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
        use crate::post::{Post, PostAction};
        for mut list in lists {
            trace!("Examining list {}", list.list_id());
            let filters = self.get_list_filters(&list);
            let memberships = self.list_members(list.pk)?;
            trace!("List members {:#?}", &memberships);
            let mut post = Post {
                policy: self.get_list_policy(list.pk)?,
                list_owners: self.get_list_owners(list.pk)?,
                list: &mut list,
                from: env.from()[0].clone(),
                memberships: &memberships,
                bytes: raw.to_vec(),
                to: env.to().to_vec(),
                action: PostAction::Hold,
            };
            let result = filters
                .into_iter()
                .fold(Ok(&mut post), |p, f| p.and_then(|p| f.feed(p)));
            trace!("result {:#?}", result);

            let Post { bytes, action, .. } = post;
            match configuration.send_mail {
                crate::config::SendMail::Smtp(ref smtp_conf) => {
                    let smtp_conf = smtp_conf.clone();
                    use melib::futures;
                    use melib::smol;
                    use melib::smtp::*;
                    std::thread::spawn(|| smol::run(futures::future::pending::<()>()));
                    let mut conn = futures::executor::block_on(SmtpConnection::new_connection(
                        smtp_conf.clone(),
                    ))?;
                    match action {
                        PostAction::Accept {
                            recipients,
                            digests: _digests,
                        } => {
                            futures::executor::block_on(conn.mail_transaction(
                                &String::from_utf8_lossy(&bytes),
                                Some(&recipients),
                            ))?;
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
        list: &MailingList,
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
                        //FIXME: send notification to list-owner
                    }
                    if let Err(_err) = self.add_member(list.pk, membership) {
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
