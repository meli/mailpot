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

use std::borrow::Cow;

use super::*;
use crate::mail::ListRequest;

impl Connection {
    /// Insert a mailing list post into the database.
    pub fn insert_post(&self, list_pk: i64, message: &[u8], env: &Envelope) -> Result<i64> {
        let from_ = env.from();
        let address = if from_.is_empty() {
            String::new()
        } else {
            from_[0].get_email()
        };
        let datetime: std::borrow::Cow<'_, str> = if env.timestamp != 0 {
            melib::datetime::timestamp_to_string(
                env.timestamp,
                Some(melib::datetime::RFC3339_FMT_WITH_TIME),
                true,
            )
            .into()
        } else {
            env.date.as_str().into()
        };
        let message_id = env.message_id_display();
        let mut stmt = self.connection.prepare(
            "INSERT OR REPLACE INTO post(list, address, message_id, message, datetime, timestamp) \
             VALUES(?, ?, ?, ?, ?, ?) RETURNING pk;",
        )?;
        let pk = stmt.query_row(
            rusqlite::params![
                &list_pk,
                &address,
                &message_id,
                &message,
                &datetime,
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

    /// Process a new mailing list post.
    pub fn post(&mut self, env: &Envelope, raw: &[u8], _dry_run: bool) -> Result<()> {
        let result = self.inner_post(env, raw, _dry_run);
        if let Err(err) = result {
            return match self.insert_to_error_queue(None, env, raw, err.to_string()) {
                Ok(idx) => {
                    log::info!(
                        "Inserted mail from {:?} into error_queue at index {}",
                        env.from(),
                        idx
                    );
                    Err(err)
                }
                Err(err2) => {
                    log::error!(
                        "Could not insert mail from {:?} into error_queue: {err2}",
                        env.from(),
                    );

                    Err(err.chain_err(|| err2))
                }
            };
        }
        result
    }

    fn inner_post(&mut self, env: &Envelope, raw: &[u8], _dry_run: bool) -> Result<()> {
        trace!("Received envelope to post: {:#?}", &env);
        let tos = env.to().to_vec();
        if tos.is_empty() {
            return Err("Envelope To: field is empty!".into());
        }
        if env.from().is_empty() {
            return Err("Envelope From: field is empty!".into());
        }
        let mut lists = self.lists()?;
        if lists.is_empty() {
            return Err("No active mailing lists found.".into());
        }
        let prev_list_len = lists.len();
        for t in &tos {
            if let Some((addr, subaddr)) = t.subaddress("+") {
                lists.retain(|list| {
                    if !addr.contains_address(&list.address()) {
                        return true;
                    }
                    if let Err(err) = ListRequest::try_from((subaddr.as_str(), env))
                        .and_then(|req| self.request(list, req, env, raw))
                    {
                        info!("Processing request returned error: {}", err);
                    }
                    false
                });
                if lists.len() != prev_list_len {
                    // Was request, handled above.
                    return Ok(());
                }
            }
        }

        lists.retain(|list| {
            trace!(
                "Is post related to list {}? {}",
                &list,
                tos.iter().any(|a| a.contains_address(&list.address()))
            );

            tos.iter().any(|a| a.contains_address(&list.address()))
        });
        if lists.is_empty() {
            return Err(format!(
                "No relevant mailing list found for these addresses: {:?}",
                tos
            )
            .into());
        }

        trace!("Configuration is {:#?}", &self.conf);
        use crate::mail::{ListContext, Post, PostAction};
        for mut list in lists {
            trace!("Examining list {}", list.display_name());
            let filters = self.list_filters(&list);
            let subscriptions = self.list_subscriptions(list.pk)?;
            let owners = self.list_owners(list.pk)?;
            trace!("List subscriptions {:#?}", &subscriptions);
            let mut list_ctx = ListContext {
                post_policy: self.list_post_policy(list.pk)?,
                subscription_policy: self.list_subscription_policy(list.pk)?,
                list_owners: &owners,
                list: &mut list,
                subscriptions: &subscriptions,
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
            trace!("Action is {:#?}", action);
            let post_env = melib::Envelope::from_bytes(&bytes, None)?;
            match action {
                PostAction::Accept => {
                    let _post_pk = self.insert_post(list_ctx.list.pk, &bytes, &post_env)?;
                    trace!("post_pk is {:#?}", _post_pk);
                    for job in list_ctx.scheduled_jobs.iter() {
                        trace!("job is {:#?}", &job);
                        if let crate::mail::MailJob::Send { recipients } = job {
                            trace!("recipients: {:?}", &recipients);
                            if recipients.is_empty() {
                                trace!("list has no recipients");
                            }
                            for recipient in recipients {
                                let mut env = post_env.clone();
                                env.set_to(melib::smallvec::smallvec![recipient.clone()]);
                                self.insert_to_queue(QueueEntry::new(
                                    Queue::Out,
                                    Some(list.pk),
                                    Some(Cow::Owned(env)),
                                    &bytes,
                                    None,
                                )?)?;
                            }
                        }
                    }
                }
                PostAction::Reject { reason } => {
                    /* FIXME - Notify submitter */
                    trace!("PostAction::Reject {{ reason: {} }}", reason);
                    //futures::executor::block_on(conn.mail_transaction(&post.bytes, b)).unwrap();
                    return Err(PostRejected(reason).into());
                }
                PostAction::Defer { reason } => {
                    trace!("PostAction::Defer {{ reason: {} }}", reason);
                    /*
                     * - FIXME Notify submitter
                     * - FIXME Save in database */
                    return Err(PostRejected(reason).into());
                }
                PostAction::Hold => {
                    trace!("PostAction::Hold");
                    /* FIXME - Save in database */
                    return Err(PostRejected("Hold".into()).into());
                }
            }
        }

        Ok(())
    }

    /// Process a new mailing list request.
    pub fn request(
        &mut self,
        list: &DbVal<MailingList>,
        request: ListRequest,
        env: &Envelope,
        raw: &[u8],
    ) -> Result<()> {
        let post_policy = self.list_post_policy(list.pk)?;
        match request {
            ListRequest::Subscribe => {
                trace!(
                    "subscribe action for addresses {:?} in list {}",
                    env.from(),
                    list
                );

                let approval_needed = post_policy
                    .as_ref()
                    .map(|p| p.approval_needed)
                    .unwrap_or(false);
                for f in env.from() {
                    let subscription = ListSubscription {
                        pk: 0,
                        list: list.pk,
                        address: f.get_email(),
                        account: None,
                        name: f.get_display_name(),
                        digest: false,
                        hide_address: false,
                        receive_duplicates: true,
                        receive_own_posts: false,
                        receive_confirmation: true,
                        enabled: !approval_needed,
                        verified: true,
                    };
                    if approval_needed {
                        match self.add_candidate_subscription(list.pk, subscription) {
                            Ok(v) => {
                                let list_owners = self.list_owners(list.pk)?;
                                self.send_reply_with_list_template(
                                    TemplateRenderContext {
                                        template: Template::SUBSCRIPTION_REQUEST_NOTICE_OWNER,
                                        default_fn: Some(
                                            Template::default_subscription_request_owner,
                                        ),
                                        list,
                                        context: minijinja::context! {
                                            list => &list,
                                            candidate => &v,
                                        },
                                        queue: Queue::Out,
                                        comment: Template::SUBSCRIPTION_REQUEST_NOTICE_OWNER
                                            .to_string(),
                                    },
                                    list_owners.iter().map(|owner| Cow::Owned(owner.address())),
                                )?;
                            }
                            Err(err) => {
                                log::error!(
                                    "Could not create candidate subscription for {f:?}: {err}"
                                );
                                /* send error notice to e-mail sender */
                                self.send_reply_with_list_template(
                                    TemplateRenderContext {
                                        template: Template::GENERIC_FAILURE,
                                        default_fn: Some(Template::default_generic_failure),
                                        list,
                                        context: minijinja::context! {
                                            list => &list,
                                        },
                                        queue: Queue::Out,
                                        comment: format!(
                                            "Could not create candidate subscription for {f:?}: \
                                             {err}"
                                        ),
                                    },
                                    std::iter::once(Cow::Borrowed(f)),
                                )?;

                                /* send error details to list owners */

                                let list_owners = self.list_owners(list.pk)?;
                                self.send_reply_with_list_template(
                                    TemplateRenderContext {
                                        template: Template::ADMIN_NOTICE,
                                        default_fn: Some(Template::default_admin_notice),
                                        list,
                                        context: minijinja::context! {
                                            list => &list,
                                            details => err.to_string(),
                                        },
                                        queue: Queue::Out,
                                        comment: format!(
                                            "Could not create candidate subscription for {f:?}: \
                                             {err}"
                                        ),
                                    },
                                    list_owners.iter().map(|owner| Cow::Owned(owner.address())),
                                )?;
                            }
                        }
                    } else if let Err(err) = self.add_subscription(list.pk, subscription) {
                        log::error!("Could not create subscription for {f:?}: {err}");

                        /* send error notice to e-mail sender */

                        self.send_reply_with_list_template(
                            TemplateRenderContext {
                                template: Template::GENERIC_FAILURE,
                                default_fn: Some(Template::default_generic_failure),
                                list,
                                context: minijinja::context! {
                                    list => &list,
                                },
                                queue: Queue::Out,
                                comment: format!("Could not create subscription for {f:?}: {err}"),
                            },
                            std::iter::once(Cow::Borrowed(f)),
                        )?;

                        /* send error details to list owners */

                        let list_owners = self.list_owners(list.pk)?;
                        self.send_reply_with_list_template(
                            TemplateRenderContext {
                                template: Template::ADMIN_NOTICE,
                                default_fn: Some(Template::default_admin_notice),
                                list,
                                context: minijinja::context! {
                                    list => &list,
                                    details => err.to_string(),
                                },
                                queue: Queue::Out,
                                comment: format!("Could not create subscription for {f:?}: {err}"),
                            },
                            list_owners.iter().map(|owner| Cow::Owned(owner.address())),
                        )?;
                    } else {
                        self.send_reply_with_list_template(
                            TemplateRenderContext {
                                template: Template::SUBSCRIPTION_CONFIRMATION,
                                default_fn: Some(Template::default_subscription_confirmation),
                                list,
                                context: minijinja::context! {
                                    list => &list,
                                },
                                queue: Queue::Out,
                                comment: Template::SUBSCRIPTION_CONFIRMATION.to_string(),
                            },
                            std::iter::once(Cow::Borrowed(f)),
                        )?;
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
                    if let Err(err) = self.remove_subscription(list.pk, &f.get_email()) {
                        log::error!("Could not unsubscribe {f:?}: {err}");
                        /* send error notice to e-mail sender */

                        self.send_reply_with_list_template(
                            TemplateRenderContext {
                                template: Template::GENERIC_FAILURE,
                                default_fn: Some(Template::default_generic_failure),
                                list,
                                context: minijinja::context! {
                                    list => &list,
                                },
                                queue: Queue::Out,
                                comment: format!("Could not unsubscribe {f:?}: {err}"),
                            },
                            std::iter::once(Cow::Borrowed(f)),
                        )?;

                        /* send error details to list owners */

                        let list_owners = self.list_owners(list.pk)?;
                        self.send_reply_with_list_template(
                            TemplateRenderContext {
                                template: Template::ADMIN_NOTICE,
                                default_fn: Some(Template::default_admin_notice),
                                list,
                                context: minijinja::context! {
                                    list => &list,
                                    details => err.to_string(),
                                },
                                queue: Queue::Out,
                                comment: format!("Could not unsubscribe {f:?}: {err}"),
                            },
                            list_owners.iter().map(|owner| Cow::Owned(owner.address())),
                        )?;
                    } else {
                        self.send_reply_with_list_template(
                            TemplateRenderContext {
                                template: Template::UNSUBSCRIPTION_CONFIRMATION,
                                default_fn: Some(Template::default_unsubscription_confirmation),
                                list,
                                context: minijinja::context! {
                                    list => &list,
                                },
                                queue: Queue::Out,
                                comment: Template::UNSUBSCRIPTION_CONFIRMATION.to_string(),
                            },
                            std::iter::once(Cow::Borrowed(f)),
                        )?;
                    }
                }
            }
            ListRequest::Other(ref req) if req == "owner" => {
                trace!(
                    "list-owner mail action for addresses {:?} in list {}",
                    env.from(),
                    list
                );
                return Err("list-owner emails are not implemented yet.".into());
                //FIXME: mail to list-owner
                /*
                for _owner in self.list_owners(list.pk)? {
                        self.insert_to_queue(
                            Queue::Out,
                            Some(list.pk),
                            None,
                            draft.finalise()?.as_bytes(),
                            "list-owner-forward".to_string(),
                        )?;
                }
                */
            }
            ListRequest::Other(ref req) if req.trim().eq_ignore_ascii_case("password") => {
                trace!(
                    "list-request password set action for addresses {:?} in list {}",
                    env.from(),
                    list
                );
                let body = env.body_bytes(raw);
                let password = body.text();
                // TODO: validate SSH public key with `ssh-keygen`.
                for f in env.from() {
                    let email_from = f.get_email();
                    if let Ok(sub) = self.list_subscription_by_address(list.pk, &email_from) {
                        match self.account_by_address(&email_from)? {
                            Some(_acc) => {
                                let changeset = AccountChangeset {
                                    address: email_from.clone(),
                                    name: None,
                                    public_key: None,
                                    password: Some(password.clone()),
                                    enabled: None,
                                };
                                self.update_account(changeset)?;
                            }
                            None => {
                                // Create new account.
                                self.add_account(Account {
                                    pk: 0,
                                    name: sub.name.clone(),
                                    address: sub.address.clone(),
                                    public_key: None,
                                    password: password.clone(),
                                    enabled: sub.enabled,
                                })?;
                            }
                        }
                    }
                }
            }
            ListRequest::RetrieveMessages(ref message_ids) => {
                trace!(
                    "retrieve messages {:?} action for addresses {:?} in list {}",
                    message_ids,
                    env.from(),
                    list
                );
                return Err("message retrievals are not implemented yet.".into());
            }
            ListRequest::RetrieveArchive(ref from, ref to) => {
                trace!(
                    "retrieve archive action from {:?} to {:?} for addresses {:?} in list {}",
                    from,
                    to,
                    env.from(),
                    list
                );
                return Err("message retrievals are not implemented yet.".into());
            }
            ListRequest::SetDigest(ref toggle) => {
                trace!(
                    "set digest action with value {} for addresses {:?} in list {}",
                    toggle,
                    env.from(),
                    list
                );
                return Err("setting digest options via e-mail is not implemented yet.".into());
            }
            ListRequest::Other(ref req) => {
                trace!(
                    "unknown request action {} for addresses {:?} in list {}",
                    req,
                    env.from(),
                    list
                );
                return Err(format!("Unknown request {req}.").into());
            }
        }
        Ok(())
    }

    /// Fetch all year and month values for which at least one post exists in
    /// `yyyy-mm` format.
    pub fn months(&self, list_pk: i64) -> Result<Vec<String>> {
        let mut stmt = self.connection.prepare(
            "SELECT DISTINCT strftime('%Y-%m', CAST(timestamp AS INTEGER), 'unixepoch') FROM post \
             WHERE list = ?;",
        )?;
        let months_iter = stmt.query_map([list_pk], |row| {
            let val: String = row.get(0)?;
            Ok(val)
        })?;

        let mut ret = vec![];
        for month in months_iter {
            let month = month?;
            ret.push(month);
        }
        Ok(ret)
    }

    /// Find a post by its `Message-ID` email header.
    pub fn list_post_by_message_id(
        &self,
        list_pk: i64,
        message_id: &str,
    ) -> Result<Option<DbVal<Post>>> {
        let mut stmt = self.connection.prepare(
            "SELECT *, strftime('%Y-%m', CAST(timestamp AS INTEGER), 'unixepoch') AS month_year \
             FROM post WHERE list = ? AND message_id = ?;",
        )?;
        let ret = stmt
            .query_row(rusqlite::params![&list_pk, &message_id], |row| {
                let pk = row.get("pk")?;
                Ok(DbVal(
                    Post {
                        pk,
                        list: row.get("list")?,
                        envelope_from: row.get("envelope_from")?,
                        address: row.get("address")?,
                        message_id: row.get("message_id")?,
                        message: row.get("message")?,
                        timestamp: row.get("timestamp")?,
                        datetime: row.get("datetime")?,
                        month_year: row.get("month_year")?,
                    },
                    pk,
                ))
            })
            .optional()?;

        Ok(ret)
    }

    /// Helper function to send a template reply.
    pub fn send_reply_with_list_template<'ctx, F: Fn() -> Template>(
        &self,
        render_context: TemplateRenderContext<'ctx, F>,
        recipients: impl Iterator<Item = Cow<'ctx, melib::Address>>,
    ) -> Result<()> {
        let TemplateRenderContext {
            template,
            default_fn,
            list,
            context,
            queue,
            comment,
        } = render_context;

        let post_policy = self.list_post_policy(list.pk)?;
        let subscription_policy = self.list_subscription_policy(list.pk)?;

        let templ = self
            .fetch_template(template, Some(list.pk))?
            .map(DbVal::into_inner)
            .or_else(|| default_fn.map(|f| f()))
            .ok_or_else(|| -> crate::Error {
                format!("Template with name {template:?} was not found.").into()
            })?;

        let mut draft = templ.render(context)?;
        draft.headers.insert(
            melib::HeaderName::new_unchecked("From"),
            list.request_subaddr(),
        );
        for addr in recipients {
            let mut draft = draft.clone();
            draft
                .headers
                .insert(melib::HeaderName::new_unchecked("To"), addr.to_string());
            list.insert_headers(
                &mut draft,
                post_policy.as_deref(),
                subscription_policy.as_deref(),
            );
            self.insert_to_queue(QueueEntry::new(
                queue,
                Some(list.pk),
                None,
                draft.finalise()?.as_bytes(),
                Some(comment.clone()),
            )?)?;
        }
        Ok(())
    }
}

/// Helper type for [`Connection::send_reply_with_list_template`].
#[derive(Debug)]
pub struct TemplateRenderContext<'ctx, F: Fn() -> Template> {
    /// Template name.
    pub template: &'ctx str,
    /// If template is not found, call a function that returns one.
    pub default_fn: Option<F>,
    /// The pertinent list.
    pub list: &'ctx DbVal<MailingList>,
    /// [`minijinja`]'s template context.
    pub context: minijinja::value::Value,
    /// Destination queue in the database.
    pub queue: Queue,
    /// Comment for the queue entry in the database.
    pub comment: String,
}
