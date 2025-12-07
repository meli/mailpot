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

//! Implementations of CLI subcommands

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
};

use clap::builder::TypedValueParser;
use mailpot::{
    melib,
    melib::{maildir::utilities::MaildirFilePathExt, smol, Envelope, EnvelopeHash},
    models::{changesets::*, *},
    queue::{Queue, QueueEntry},
    transaction::TransactionBehavior,
    Connection, Context, Error, ErrorKind, Result,
};

use crate::{args::*, import, lints::*};

macro_rules! list {
    ($db:ident, $list_id:expr) => {{
        $db.list_by_id(&$list_id)?.or_else(|| {
            $list_id
                .parse::<i64>()
                .ok()
                .map(|pk| $db.list(pk).ok())
                .flatten()
                .flatten()
        })
    }};
}

macro_rules! string_opts {
    ($field:ident) => {
        if $field.as_deref().map(str::is_empty).unwrap_or(false) {
            None
        } else {
            Some($field)
        }
    };
}

pub fn dump_database(db: &mut Connection) -> Result<()> {
    let lists = db.lists()?;
    let mut stdout = std::io::stdout();
    serde_json::to_writer_pretty(&mut stdout, &lists)?;
    for l in &lists {
        serde_json::to_writer_pretty(
            &mut stdout,
            &db.list_subscriptions(l.pk)
                .context("Could not retrieve list subscriptions.")?,
        )?;
    }
    Ok(())
}

pub fn list_lists(db: &mut Connection) -> Result<()> {
    let lists = db.lists().context("Could not retrieve lists.")?;
    if lists.is_empty() {
        println!("No lists found.");
    } else {
        for l in lists {
            println!("- {} {:?}", l.id, l);
            let list_owners = db
                .list_owners(l.pk)
                .context("Could not retrieve list owners.")?;
            if list_owners.is_empty() {
                println!("\tList owners: None");
            } else {
                println!("\tList owners:");
                for o in list_owners {
                    println!("\t- {}", o);
                }
            }
            if let Some(s) = db
                .list_post_policy(l.pk)
                .context("Could not retrieve list post policy.")?
            {
                println!("\tPost policy: {}", s);
            } else {
                println!("\tPost policy: None");
            }
            if let Some(s) = db
                .list_subscription_policy(l.pk)
                .context("Could not retrieve list subscription policy.")?
            {
                println!("\tSubscription policy: {}", s);
            } else {
                println!("\tSubscription policy: None");
            }
            println!();
        }
    }
    Ok(())
}

pub fn list(db: &mut Connection, list_id: &str, cmd: ListCommand, quiet: bool) -> Result<()> {
    let list = match list!(db, list_id) {
        Some(v) => v,
        None => {
            return Err(format!("No list with id or pk {} was found", list_id).into());
        }
    };
    use ListCommand::*;
    match cmd {
        Subscriptions => {
            let subscriptions = db.list_subscriptions(list.pk)?;
            if subscriptions.is_empty() {
                if !quiet {
                    println!("No subscriptions found.");
                }
            } else {
                if !quiet {
                    println!("Subscriptions of list {}", list.id);
                }
                for l in subscriptions {
                    println!("- {}", &l);
                }
            }
        }
        AddSubscription {
            address,
            subscription_options:
                SubscriptionOptions {
                    name,
                    digest,
                    hide_address,
                    receive_duplicates,
                    receive_own_posts,
                    receive_confirmation,
                    enabled,
                    verified,
                },
        } => {
            db.add_subscription(
                list.pk,
                ListSubscription {
                    pk: 0,
                    list: list.pk,
                    address,
                    account: None,
                    name,
                    digest: digest.unwrap_or(false),
                    hide_address: hide_address.unwrap_or(false),
                    receive_confirmation: receive_confirmation.unwrap_or(true),
                    receive_duplicates: receive_duplicates.unwrap_or(true),
                    receive_own_posts: receive_own_posts.unwrap_or(false),
                    enabled: enabled.unwrap_or(true),
                    verified: verified.unwrap_or(false),
                },
            )?;
        }
        RemoveSubscription { address } => {
            let mut input = String::new();
            loop {
                println!(
                    "Are you sure you want to remove subscription of {} from list {}? [Yy/n]",
                    address, list
                );
                input.clear();
                std::io::stdin().read_line(&mut input)?;
                if input.trim() == "Y" || input.trim() == "y" || input.trim() == "" {
                    break;
                } else if input.trim() == "n" {
                    return Ok(());
                }
            }

            db.remove_subscription(list.pk, &address)?;
        }
        Health => {
            if !quiet {
                println!("{} health:", list);
            }
            let list_owners = db
                .list_owners(list.pk)
                .context("Could not retrieve list owners.")?;
            let post_policy = db
                .list_post_policy(list.pk)
                .context("Could not retrieve list post policy.")?;
            let subscription_policy = db
                .list_subscription_policy(list.pk)
                .context("Could not retrieve list subscription policy.")?;
            if list_owners.is_empty() {
                println!("\tList has no owners: you should add at least one.");
            } else {
                for owner in list_owners {
                    println!("\tList owner: {}.", owner);
                }
            }
            if let Some(p) = post_policy {
                println!("\tList has post policy: {p}.");
            } else {
                println!("\tList has no post policy: you should add one.");
            }
            if let Some(p) = subscription_policy {
                println!("\tList has subscription policy: {p}.");
            } else {
                println!("\tList has no subscription policy: you should add one.");
            }
        }
        Info => {
            println!("{} info:", list);
            let list_owners = db
                .list_owners(list.pk)
                .context("Could not retrieve list owners.")?;
            let post_policy = db
                .list_post_policy(list.pk)
                .context("Could not retrieve list post policy.")?;
            let subscription_policy = db
                .list_subscription_policy(list.pk)
                .context("Could not retrieve list subscription policy.")?;
            let subscriptions = db
                .list_subscriptions(list.pk)
                .context("Could not retrieve list subscriptions.")?;
            if subscriptions.is_empty() {
                println!("No subscriptions.");
            } else if subscriptions.len() == 1 {
                println!("1 subscription.");
            } else {
                println!("{} subscriptions.", subscriptions.len());
            }
            if list_owners.is_empty() {
                println!("List owners: None");
            } else {
                println!("List owners:");
                for o in list_owners {
                    println!("\t- {}", o);
                }
            }
            if let Some(s) = post_policy {
                println!("Post policy: {s}");
            } else {
                println!("Post policy: None");
            }
            if let Some(s) = subscription_policy {
                println!("Subscription policy: {s}");
            } else {
                println!("Subscription policy: None");
            }
        }
        UpdateSubscription {
            address,
            subscription_options:
                SubscriptionOptions {
                    name,
                    digest,
                    hide_address,
                    receive_duplicates,
                    receive_own_posts,
                    receive_confirmation,
                    enabled,
                    verified,
                },
        } => {
            let name = if name
                .as_ref()
                .map(|s: &String| s.is_empty())
                .unwrap_or(false)
            {
                None
            } else {
                Some(name)
            };
            let changeset = ListSubscriptionChangeset {
                list: list.pk,
                address,
                account: None,
                name,
                digest,
                verified,
                hide_address,
                receive_duplicates,
                receive_own_posts,
                receive_confirmation,
                enabled,
            };
            db.update_subscription(changeset)?;
        }
        AddPostPolicy {
            announce_only,
            subscription_only,
            approval_needed,
            open,
            custom,
        } => {
            let policy = PostPolicy {
                pk: 0,
                list: list.pk,
                announce_only,
                subscription_only,
                approval_needed,
                open,
                custom,
            };
            let new_val = db.set_list_post_policy(policy)?;
            println!("Added new policy with pk = {}", new_val.pk());
        }
        RemovePostPolicy { pk } => {
            db.remove_list_post_policy(list.pk, pk)?;
            println!("Removed policy with pk = {}", pk);
        }
        AddSubscriptionPolicy {
            send_confirmation,
            open,
            manual,
            request,
            custom,
        } => {
            let policy = SubscriptionPolicy {
                pk: 0,
                list: list.pk,
                send_confirmation,
                open,
                manual,
                request,
                custom,
            };
            let new_val = db.set_list_subscription_policy(policy)?;
            println!("Added new subscribe policy with pk = {}", new_val.pk());
        }
        RemoveSubscriptionPolicy { pk } => {
            db.remove_list_subscription_policy(list.pk, pk)?;
            println!("Removed subscribe policy with pk = {}", pk);
        }
        AddListOwner { address, name } => {
            let list_owner = ListOwner {
                pk: 0,
                list: list.pk,
                address,
                name,
            };
            let new_val = db.add_list_owner(list_owner)?;
            println!("Added new list owner {}", new_val);
        }
        RemoveListOwner { pk } => {
            db.remove_list_owner(list.pk, pk)?;
            println!("Removed list owner with pk = {}", pk);
        }
        EnableSubscription { address } => {
            let changeset = ListSubscriptionChangeset {
                list: list.pk,
                address,
                account: None,
                name: None,
                digest: None,
                verified: None,
                enabled: Some(true),
                hide_address: None,
                receive_duplicates: None,
                receive_own_posts: None,
                receive_confirmation: None,
            };
            db.update_subscription(changeset)?;
        }
        DisableSubscription { address } => {
            let changeset = ListSubscriptionChangeset {
                list: list.pk,
                address,
                account: None,
                name: None,
                digest: None,
                enabled: Some(false),
                verified: None,
                hide_address: None,
                receive_duplicates: None,
                receive_own_posts: None,
                receive_confirmation: None,
            };
            db.update_subscription(changeset)?;
        }
        Update {
            name,
            id,
            address,
            description,
            archive_url,
            owner_local_part,
            request_local_part,
            verify,
            hidden,
            enabled,
        } => {
            let description = string_opts!(description);
            let archive_url = string_opts!(archive_url);
            let owner_local_part = string_opts!(owner_local_part);
            let request_local_part = string_opts!(request_local_part);
            let changeset = MailingListChangeset {
                pk: list.pk,
                name,
                id,
                address,
                description,
                archive_url,
                owner_local_part,
                request_local_part,
                verify,
                hidden,
                enabled,
            };
            db.update_list(changeset)?;
        }
        ImportMembers {
            url,
            username,
            password,
            list_id,
            dry_run,
            skip_owners,
        } => {
            let conn = import::Mailman3Connection::new(&url, &username, &password).unwrap();
            if dry_run {
                let entries = conn.users(&list_id).unwrap();
                println!("{} result(s)", entries.len());
                for e in entries {
                    println!(
                        "{}{}<{}>",
                        e.display_name().unwrap_or_default(),
                        if e.display_name().is_none() { "" } else { " " },
                        e.email()
                    );
                }
                if !skip_owners {
                    let entries = conn.owners(&list_id).unwrap();
                    println!("\nOwners: {} result(s)", entries.len());
                    for e in entries {
                        println!(
                            "{}{}<{}>",
                            e.display_name().unwrap_or_default(),
                            if e.display_name().is_none() { "" } else { " " },
                            e.email()
                        );
                    }
                }
            } else {
                let entries = conn.users(&list_id).unwrap();
                let tx = db.transaction(Default::default()).unwrap();
                for sub in entries.into_iter().map(|e| e.into_subscription(list.pk)) {
                    tx.add_subscription(list.pk, sub)?;
                }
                if !skip_owners {
                    let entries = conn.owners(&list_id).unwrap();
                    for sub in entries.into_iter().map(|e| e.into_owner(list.pk)) {
                        tx.add_list_owner(sub)?;
                    }
                }
                tx.commit()?;
            }
        }
        SubscriptionRequests => {
            let subscriptions = db.list_subscription_requests(list.pk)?;
            if subscriptions.is_empty() {
                println!("No subscription requests found.");
            } else {
                println!("Subscription requests of list {}", list.id);
                for l in subscriptions {
                    println!("- {}", &l);
                }
            }
        }
        AcceptSubscriptionRequest {
            pk,
            do_not_send_confirmation,
        } => match db.accept_candidate_subscription(pk) {
            Ok(subscription) => {
                println!("Added: {subscription:#?}");
                if !do_not_send_confirmation {
                    if let Err(err) = db
                        .list(subscription.list)
                        .and_then(|v| match v {
                            Some(v) => Ok(v),
                            None => Err(format!(
                                "No list with id or pk {} was found",
                                subscription.list
                            )
                            .into()),
                        })
                        .and_then(|list| {
                            db.send_subscription_confirmation(&list, &subscription.address())
                        })
                    {
                        eprintln!("Could not send subscription confirmation!");
                        return Err(err);
                    }
                    println!("Sent confirmation e-mail to {}", subscription.address());
                } else {
                    println!(
                        "Did not sent confirmation e-mail to {}. You can do it manually with the \
                         appropriate command.",
                        subscription.address()
                    );
                }
            }
            Err(err) => {
                eprintln!("Could not accept subscription request!");
                return Err(err);
            }
        },
        SendConfirmationForSubscription { pk } => {
            let req = match db.candidate_subscription(pk) {
                Ok(req) => req,
                Err(err) => {
                    eprintln!("Could not find subscription request by that pk!");

                    return Err(err);
                }
            };
            log::info!("Found {:#?}", req);
            if req.accepted.is_none() {
                return Err("Request has not been accepted!".into());
            }
            if let Err(err) = db
                .list(req.list)
                .and_then(|v| match v {
                    Some(v) => Ok(v),
                    None => Err(format!("No list with id or pk {} was found", req.list).into()),
                })
                .and_then(|list| db.send_subscription_confirmation(&list, &req.address()))
            {
                eprintln!("Could not send subscription request confirmation!");
                return Err(err);
            }

            println!("Sent confirmation e-mail to {}", req.address());
        }
        PrintMessageFilterSettings {
            show_available: true,
            ref filter,
        } => {
            use crate::message_filter_settings::MessageFilterSettingNameValueParser;
            let value_parser = MessageFilterSettingNameValueParser;
            let Some(possible_values) =
                <MessageFilterSettingNameValueParser as TypedValueParser>::possible_values(
                    &value_parser,
                )
            else {
                println!("No settings available.");
                return Ok(());
            };
            let mut possible_values = possible_values.into_iter().collect::<Vec<_>>();
            possible_values.sort_by(|a, b| a.get_name().partial_cmp(b.get_name()).unwrap());
            for val in possible_values.into_iter().filter(|val| {
                let Some(filter) = filter.as_ref() else {
                    return true;
                };
                val.matches(filter, true)
            }) {
                println!("{}", val.get_name());
            }
        }
        PrintMessageFilterSettings {
            show_available: false,
            filter,
        } => {
            let mut settings = db
                .get_settings(list.pk())?
                .into_iter()
                .collect::<Vec<(_, _)>>();
            settings.sort_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap());
            for (name, value) in settings.iter().filter(|(name, _)| {
                let Some(filter) = filter.as_ref() else {
                    return true;
                };
                name.to_ascii_lowercase()
                    .contains(&filter.to_ascii_lowercase())
            }) {
                println!("{}: {}", name, value);
            }
        }
        SetMessageFilterSetting { name, value } => {
            let value = serde_json::from_str(&value).map_err(|err| {
                ErrorKind::External(mailpot::anyhow::anyhow!(format!(
                    "Provided value is not valid json: {}",
                    err
                )))
            })?;
            db.set_settings(list.pk(), &name.to_string(), value)?;
            if !quiet {
                println!(
                    "Successfully updated {} value for list {}.",
                    name, list.name
                );
            }
        }
    }
    Ok(())
}

pub fn create_list(
    db: &mut Connection,
    name: String,
    id: String,
    address: String,
    description: Option<String>,
    archive_url: Option<String>,
    quiet: bool,
) -> Result<()> {
    let new = db.create_list(MailingList {
        pk: 0,
        name,
        id,
        description,
        topics: vec![],
        address,
        archive_url,
    })?;
    log::trace!("created new list {:#?}", new);
    if !quiet {
        println!(
            "Created new list {:?} with primary key {}",
            new.id,
            new.pk()
        );
    }
    Ok(())
}

pub fn post(db: &mut Connection, dry_run: bool, debug: bool) -> Result<()> {
    if debug {
        println!("Post dry_run = {:?}", dry_run);
    }

    let tx = db
        .transaction(TransactionBehavior::Exclusive)
        .context("Could not open Exclusive transaction in database.")?;
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("Could not read from stdin")?;
    match Envelope::from_bytes(input.as_bytes(), None) {
        Ok(env) => {
            if debug {
                eprintln!("Parsed envelope is:\n{:?}", &env);
            }
            tx.post(&env, input.as_bytes(), dry_run)?;
        }
        Err(err) if input.trim().is_empty() => {
            eprintln!("Empty input, abort.");
            return Err(err.into());
        }
        Err(err) => {
            eprintln!("Could not parse message: {}", err);
            let p = tx.conf().save_message(input)?;
            eprintln!("Message saved at {}", p.display());
            return Err(err.into());
        }
    }
    tx.commit()
}

pub fn flush_queue(db: &mut Connection, dry_run: bool, verbose: u8, debug: bool) -> Result<()> {
    let tx = db
        .transaction(TransactionBehavior::Exclusive)
        .context("Could not open Exclusive transaction in database.")?;
    let messages = tx.delete_from_queue(mailpot::queue::Queue::Out, vec![])?;
    if verbose > 0 || debug {
        println!("Queue out has {} messages.", messages.len());
    }

    let mut failures = Vec::with_capacity(messages.len());

    let send_mail = tx.conf().send_mail.clone();
    match send_mail {
        mailpot::SendMail::ShellCommand(cmd) => {
            fn submit(cmd: &str, msg: &QueueEntry, dry_run: bool) -> Result<()> {
                if dry_run {
                    return Ok(());
                }
                let mut child = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .env("TO_ADDRESS", msg.to_addresses.clone())
                    .stdout(Stdio::piped())
                    .stdin(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .context("sh command failed to start")?;
                let mut stdin = child
                    .stdin
                    .take()
                    .ok_or_else(|| Error::from("Failed to open stdin"))?;

                let builder = std::thread::Builder::new();

                std::thread::scope(|s| {
                    let handler = builder
                        .spawn_scoped(s, move || {
                            stdin
                                .write_all(&msg.message)
                                .expect("Failed to write to stdin");
                        })
                        .context(
                            "Could not spawn IPC communication thread for SMTP ShellCommand \
                             process",
                        )?;

                    handler.join().map_err(|_| {
                        ErrorKind::External(mailpot::anyhow::anyhow!(
                            "Could not join with IPC communication thread for SMTP ShellCommand \
                             process"
                        ))
                    })?;
                    let result = child.wait_with_output()?;
                    if !result.status.success() {
                        return Err(Error::new_external(format!(
                            "{} process failed with exit code: {:?}\n{}",
                            cmd,
                            result.status.code(),
                            String::from_utf8(result.stderr).unwrap()
                        )));
                    }
                    Ok::<(), Error>(())
                })?;
                Ok(())
            }
            for msg in messages {
                if let Err(err) = submit(&cmd, &msg, dry_run) {
                    if verbose > 0 || debug {
                        eprintln!("Message {msg:?} failed with: {err}.");
                    }
                    failures.push((err, msg));
                } else if verbose > 0 || debug {
                    eprintln!("Submitted message {}", msg.message_id);
                }
            }
        }
        mailpot::SendMail::Smtp(_) => {
            let conn_future = tx.new_smtp_connection()?;
            failures = smol::future::block_on(smol::spawn(async move {
                let mut conn = conn_future.await?;
                for msg in messages {
                    if let Err(err) = Connection::submit(&mut conn, &msg, dry_run).await {
                        failures.push((err, msg));
                    }
                }
                Ok::<_, Error>(failures)
            }))?;
        }
    }

    for (err, mut msg) in failures {
        log::error!("Message {msg:?} failed with: {err}. Inserting to Deferred queue.");

        msg.queue = mailpot::queue::Queue::Deferred;
        tx.insert_to_queue(msg)?;
    }

    if !dry_run {
        tx.commit()?;
    }
    Ok(())
}

pub fn queue_(db: &mut Connection, queue: Queue, cmd: QueueCommand, quiet: bool) -> Result<()> {
    match cmd {
        QueueCommand::List => {
            let entries = db.queue(queue)?;
            if entries.is_empty() {
                if !quiet {
                    println!("Queue {queue} is empty.");
                }
            } else {
                for e in entries {
                    println!(
                        "- {} {} {} {} {}",
                        e.pk, e.datetime, e.from_address, e.to_addresses, e.subject
                    );
                }
            }
        }
        QueueCommand::Print { index } => {
            let mut entries = db.queue(queue)?;
            if !index.is_empty() {
                entries.retain(|el| index.contains(&el.pk()));
            }
            if entries.is_empty() {
                if !quiet {
                    println!("Queue {queue} is empty.");
                }
            } else {
                for e in entries {
                    println!("{e:?}");
                }
            }
        }
        QueueCommand::Delete { index } => {
            let mut entries = db.queue(queue)?;
            if !index.is_empty() {
                entries.retain(|el| index.contains(&el.pk()));
            }
            if entries.is_empty() {
                if !quiet {
                    println!("Queue {queue} is empty.");
                }
            } else {
                if !quiet {
                    println!("Deleting queue {queue} elements {:?}", &index);
                }
                db.delete_from_queue(queue, index)?;
                if !quiet {
                    for e in entries {
                        println!("{e:?}");
                    }
                }
            }
        }
    }
    Ok(())
}

// [ref:TODO]: verify config works with an integration test
fn make_config(path: &Path) -> mailpot::melib::maildir::Configuration {
    mailpot::melib::maildir::Configuration {
        path: path.to_path_buf(),
        ..Default::default()
    }
}

pub fn import_maildir(
    db: &mut Connection,
    list_id: &str,
    mut maildir_path: PathBuf,
    quiet: bool,
    debug: bool,
    verbose: u8,
) -> Result<()> {
    let list = match list!(db, list_id) {
        Some(v) => v,
        None => {
            return Err(format!("No list with id or pk {} was found", list_id).into());
        }
    };
    if !maildir_path.is_absolute() {
        maildir_path = std::env::current_dir()
            .context("could not detect current directory")?
            .join(&maildir_path);
    }

    fn get_file_hash(file: &std::path::Path) -> EnvelopeHash {
        let mut hasher = DefaultHasher::default();
        file.hash(&mut hasher);
        EnvelopeHash(hasher.finish())
    }
    let mut buf = Vec::with_capacity(4096);
    let files = melib::maildir::MaildirType::list_mail_in_maildir_fs(
        &make_config(&maildir_path),
        maildir_path,
        true,
    )
    .context("Could not parse files in maildir path")?;
    let mut ctr = 0;
    for file in files {
        let hash = get_file_hash(&file);
        let mut reader = std::io::BufReader::new(
            std::fs::File::open(&file)
                .with_context(|| format!("Could not open {}.", file.display()))?,
        );
        buf.clear();
        reader
            .read_to_end(&mut buf)
            .with_context(|| format!("Could not read from {}.", file.display()))?;
        match Envelope::from_bytes(buf.as_slice(), Some(file.flags())) {
            Ok(mut env) => {
                env.set_hash(hash);
                if verbose > 1 {
                    println!(
                        "Inserting post from {:?} with subject `{}` and Message-ID `{}`.",
                        env.from(),
                        env.subject(),
                        env.message_id()
                    );
                }
                db.insert_post(list.pk, &buf, &env).with_context(|| {
                    format!(
                        "Could not insert post `{}` from path `{}`",
                        env.message_id(),
                        file.display()
                    )
                })?;
                ctr += 1;
            }
            Err(err) => {
                if verbose > 0 || debug {
                    log::error!(
                        "Could not parse Envelope from file {}: {err}",
                        file.display()
                    );
                }
            }
        }
    }
    if !quiet {
        println!("Inserted {} posts to {}.", ctr, list_id);
    }
    Ok(())
}

pub fn update_postfix_config(
    config_path: &Path,
    db: &mut Connection,
    master_cf: Option<PathBuf>,
    PostfixConfig {
        user,
        group,
        binary_path,
        process_limit,
        map_output_path,
        transport_name,
    }: PostfixConfig,
) -> Result<()> {
    let pfconf = mailpot::postfix::PostfixConfiguration {
        user: user.into(),
        group: group.map(Into::into),
        binary_path,
        process_limit,
        map_output_path,
        transport_name: transport_name.map(std::borrow::Cow::from),
    };
    pfconf
        .save_maps(db.conf())
        .context("Could not save maps.")?;
    pfconf
        .save_master_cf_entry(db.conf(), config_path, master_cf.as_deref())
        .context("Could not save master.cf file.")?;

    Ok(())
}

pub fn print_postfix_config(
    config_path: &Path,
    db: &mut Connection,
    PostfixConfig {
        user,
        group,
        binary_path,
        process_limit,
        map_output_path,
        transport_name,
    }: PostfixConfig,
) -> Result<()> {
    let pfconf = mailpot::postfix::PostfixConfiguration {
        user: user.into(),
        group: group.map(Into::into),
        binary_path,
        process_limit,
        map_output_path,
        transport_name: transport_name.map(std::borrow::Cow::from),
    };
    let lists = db.lists().context("Could not retrieve lists.")?;
    let lists_post_policies = lists
        .into_iter()
        .map(|l| {
            let pk = l.pk;
            Ok((
                l,
                db.list_post_policy(pk).with_context(|| {
                    format!("Could not retrieve list post policy for list_pk = {pk}.")
                })?,
            ))
        })
        .collect::<Result<Vec<(DbVal<MailingList>, Option<DbVal<PostPolicy>>)>>>()?;
    let maps = pfconf.generate_maps(&lists_post_policies);
    let mastercf = pfconf.generate_master_cf_entry(db.conf(), config_path);

    println!("{maps}\n\n{mastercf}\n");
    Ok(())
}

pub fn accounts(db: &mut Connection, quiet: bool) -> Result<()> {
    let accounts = db.accounts()?;
    if accounts.is_empty() {
        if !quiet {
            println!("No accounts found.");
        }
    } else {
        for a in accounts {
            println!("- {:?}", a);
        }
    }
    Ok(())
}

pub fn account_info(db: &mut Connection, address: &str, quiet: bool) -> Result<()> {
    if let Some(acc) = db.account_by_address(address)? {
        let subs = db
            .account_subscriptions(acc.pk())
            .context("Could not retrieve account subscriptions for this account.")?;
        if subs.is_empty() {
            if !quiet {
                println!("No subscriptions found.");
            }
        } else {
            for s in subs {
                let list = db
                    .list(s.list)
                    .with_context(|| {
                        format!(
                            "Found subscription with list_pk = {} but could not retrieve the \
                             list.\nListSubscription = {:?}",
                            s.list, s
                        )
                    })?
                    .ok_or_else(|| {
                        format!(
                            "Found subscription with list_pk = {} but no such list \
                             exists.\nListSubscription = {:?}",
                            s.list, s
                        )
                    })?;
                println!("- {:?} {}", s, list);
            }
        }
    } else {
        return Err(format!("Account with address {address} not found!").into());
    }
    Ok(())
}

pub fn add_account(
    db: &mut Connection,
    address: String,
    password: String,
    name: Option<String>,
    public_key: Option<String>,
    enabled: Option<bool>,
) -> Result<()> {
    db.add_account(Account {
        pk: 0,
        name,
        address,
        public_key,
        password,
        enabled: enabled.unwrap_or(true),
    })?;
    Ok(())
}

pub fn remove_account(db: &mut Connection, address: &str, quiet: bool) -> Result<()> {
    let mut input = String::new();
    if !quiet {
        loop {
            println!(
                "Are you sure you want to remove account with address {}? [Yy/n]",
                address
            );
            input.clear();
            std::io::stdin().read_line(&mut input)?;
            if input.trim() == "Y" || input.trim() == "y" || input.trim() == "" {
                break;
            } else if input.trim() == "n" {
                return Ok(());
            }
        }
    }

    db.remove_account(address)?;

    Ok(())
}

pub fn update_account(
    db: &mut Connection,
    address: String,
    password: Option<String>,
    name: Option<Option<String>>,
    public_key: Option<Option<String>>,
    enabled: Option<Option<bool>>,
) -> Result<()> {
    let changeset = AccountChangeset {
        address,
        name,
        public_key,
        password,
        enabled,
    };
    db.update_account(changeset)?;
    Ok(())
}

pub struct RepairConfig {
    pub datetime_header_value: bool,
    pub remove_empty_accounts: bool,
    pub remove_accepted_subscription_requests: bool,
    pub warn_list_no_owner: bool,
    pub fix_message_ids: bool,
}

pub fn repair(db: &mut Connection, fix: bool, all: bool, mut config: RepairConfig) -> Result<()> {
    type LintFn = fn(&'_ mut mailpot::Connection, bool) -> std::result::Result<(), mailpot::Error>;
    let dry_run = !fix;
    if all {
        config.datetime_header_value = true;
        config.remove_empty_accounts = true;
        config.remove_accepted_subscription_requests = true;
        config.warn_list_no_owner = true;
        config.fix_message_ids = true;
    }

    if !(config.datetime_header_value
        | config.remove_empty_accounts
        | config.remove_accepted_subscription_requests
        | config.warn_list_no_owner
        | config.fix_message_ids)
    {
        return Err("No lints selected: specify them with flag arguments. See --help".into());
    }

    if dry_run {
        println!("running without making modifications (dry run)");
    }

    for (name, flag, lint_fn) in [
        (
            "datetime_header_value",
            config.datetime_header_value,
            datetime_header_value_lint as LintFn,
        ),
        (
            "remove_empty_accounts",
            config.remove_empty_accounts,
            remove_empty_accounts_lint as _,
        ),
        (
            "remove_accepted_subscription_requests",
            config.remove_accepted_subscription_requests,
            remove_accepted_subscription_requests_lint as _,
        ),
        (
            "warn_list_no_owner",
            config.warn_list_no_owner,
            warn_list_no_owner_lint as _,
        ),
        (
            "fix_message_ids",
            config.fix_message_ids,
            fix_message_ids_lint as _,
        ),
    ] {
        if flag {
            lint_fn(db, dry_run).with_context(|| format!("Lint {name} failed."))?;
        }
    }
    Ok(())
}
