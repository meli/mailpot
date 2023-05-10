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

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io::{Read, Write},
    process::Stdio,
};

mod lints;
use lints::*;
use mailpot::{
    melib::{backends::maildir::MaildirPathTrait, smol, smtp::*, Envelope, EnvelopeHash},
    models::{changesets::*, *},
    queue::{Queue, QueueEntry},
    transaction::TransactionBehavior,
    Configuration, Connection, Error, ErrorKind, Result, *,
};
use mailpot_cli::*;

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

fn run_app(opt: Opt) -> Result<()> {
    if opt.debug {
        println!("DEBUG: {:?}", &opt);
    }
    if let Command::SampleConfig { with_smtp } = opt.cmd {
        let mut new = Configuration::new("/path/to/sqlite.db");
        new.administrators.push("admin@example.com".to_string());
        if with_smtp {
            new.send_mail = mailpot::SendMail::Smtp(SmtpServerConf {
                hostname: "mail.example.com".to_string(),
                port: 587,
                envelope_from: "".to_string(),
                auth: SmtpAuth::Auto {
                    username: "user".to_string(),
                    password: Password::Raw("hunter2".to_string()),
                    auth_type: SmtpAuthType::default(),
                    require_auth: true,
                },
                security: SmtpSecurity::StartTLS {
                    danger_accept_invalid_certs: false,
                },
                extensions: Default::default(),
            });
        }
        println!("{}", new.to_toml());
        return Ok(());
    };
    let config_path = if let Some(path) = opt.config.as_ref() {
        path.as_path()
    } else {
        let mut opt = Opt::command();
        opt.error(
            clap::error::ErrorKind::MissingRequiredArgument,
            "--config is required for mailing list operations",
        )
        .exit();
    };

    let config = Configuration::from_file(config_path)?;

    use Command::*;
    let mut db = Connection::open_or_create_db(config)?.trusted();
    match opt.cmd {
        SampleConfig { .. } => {}
        DumpDatabase => {
            let lists = db.lists()?;
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &lists)?;
            for l in &lists {
                serde_json::to_writer_pretty(&mut stdout, &db.list_subscriptions(l.pk)?)?;
            }
        }
        ListLists => {
            let lists = db.lists()?;
            if lists.is_empty() {
                println!("No lists found.");
            } else {
                for l in lists {
                    println!("- {} {:?}", l.id, l);
                    let list_owners = db.list_owners(l.pk)?;
                    if list_owners.is_empty() {
                        println!("\tList owners: None");
                    } else {
                        println!("\tList owners:");
                        for o in list_owners {
                            println!("\t- {}", o);
                        }
                    }
                    if let Some(s) = db.list_post_policy(l.pk)? {
                        println!("\tList policy: {}", s);
                    } else {
                        println!("\tList policy: None");
                    }
                    println!();
                }
            }
        }
        List { list_id, cmd } => {
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
                        println!("No subscriptions found.");
                    } else {
                        println!("Subscriptions of list {}", list.id);
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
                            "Are you sure you want to remove subscription of {} from list {}? \
                             [Yy/n]",
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
                    println!("{} health:", list);
                    let list_owners = db.list_owners(list.pk)?;
                    let post_policy = db.list_post_policy(list.pk)?;
                    let subscription_policy = db.list_subscription_policy(list.pk)?;
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
                    let list_owners = db.list_owners(list.pk)?;
                    let post_policy = db.list_post_policy(list.pk)?;
                    let subscription_policy = db.list_subscription_policy(list.pk)?;
                    let subscriptions = db.list_subscriptions(list.pk)?;
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
                AddPolicy {
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
                RemovePolicy { pk } => {
                    db.remove_list_post_policy(list.pk, pk)?;
                    println!("Removed policy with pk = {}", pk);
                }
                AddSubscribePolicy {
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
                RemoveSubscribePolicy { pk } => {
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
                                if let Some(n) = e.display_name() {
                                    n
                                } else {
                                    ""
                                },
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
                                    if let Some(n) = e.display_name() {
                                        n
                                    } else {
                                        ""
                                    },
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
            }
        }
        CreateList {
            name,
            id,
            address,
            description,
            archive_url,
        } => {
            let new = db.create_list(MailingList {
                pk: 0,
                name,
                id,
                description,
                address,
                archive_url,
            })?;
            log::trace!("created new list {:#?}", new);
            if !opt.quiet {
                println!(
                    "Created new list {:?} with primary key {}",
                    new.id,
                    new.pk()
                );
            }
        }
        Post { dry_run } => {
            if opt.debug {
                println!("post dry_run{:?}", dry_run);
            }

            let tx = db.transaction(TransactionBehavior::Exclusive).unwrap();
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            match Envelope::from_bytes(input.as_bytes(), None) {
                Ok(env) => {
                    if opt.debug {
                        eprintln!("{:?}", &env);
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
            tx.commit()?;
        }
        FlushQueue { dry_run } => {
            let tx = db.transaction(TransactionBehavior::Exclusive).unwrap();
            let messages = if opt.debug {
                println!("flush-queue dry_run {:?}", dry_run);
                tx.queue(Queue::Out)?
                    .into_iter()
                    .map(DbVal::into_inner)
                    .chain(
                        tx.queue(Queue::Deferred)?
                            .into_iter()
                            .map(DbVal::into_inner),
                    )
                    .collect()
            } else {
                tx.delete_from_queue(Queue::Out, vec![])?
            };
            if opt.verbose > 0 || opt.debug {
                println!("Queue out has {} messages.", messages.len());
            }

            let mut failures = Vec::with_capacity(messages.len());

            let send_mail = tx.conf().send_mail.clone();
            match send_mail {
                mailpot::SendMail::ShellCommand(cmd) => {
                    fn submit(cmd: &str, msg: &QueueEntry) -> Result<()> {
                        let mut child = std::process::Command::new("sh")
                            .arg("-c")
                            .arg(cmd)
                            .stdout(Stdio::piped())
                            .stdin(Stdio::piped())
                            .stderr(Stdio::piped())
                            .spawn()
                            .context("sh command failed to start")?;
                        let mut stdin = child.stdin.take().context("Failed to open stdin")?;

                        let builder = std::thread::Builder::new();

                        std::thread::scope(|s| {
                            let handler = builder
                                .spawn_scoped(s, move || {
                                    stdin
                                        .write_all(&msg.message)
                                        .expect("Failed to write to stdin");
                                })
                                .context(
                                    "Could not spawn IPC communication thread for SMTP \
                                     ShellCommand process",
                                )?;

                            handler.join().map_err(|_| {
                                ErrorKind::External(mailpot::anyhow::anyhow!(
                                    "Could not join with IPC communication thread for SMTP \
                                     ShellCommand process"
                                ))
                            })?;
                            Ok::<(), Error>(())
                        })?;
                        Ok(())
                    }
                    for msg in messages {
                        if let Err(err) = submit(&cmd, &msg) {
                            failures.push((err, msg));
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

                msg.queue = Queue::Deferred;
                tx.insert_to_queue(msg)?;
            }

            tx.commit()?;
        }
        ErrorQueue { cmd } => match cmd {
            ErrorQueueCommand::List => {
                let errors = db.queue(Queue::Error)?;
                if errors.is_empty() {
                    println!("Error queue is empty.");
                } else {
                    for e in errors {
                        println!(
                            "- {} {} {} {} {}",
                            e.pk, e.datetime, e.from_address, e.to_addresses, e.subject
                        );
                    }
                }
            }
            ErrorQueueCommand::Print { index } => {
                let mut errors = db.queue(Queue::Error)?;
                if !index.is_empty() {
                    errors.retain(|el| index.contains(&el.pk()));
                }
                if errors.is_empty() {
                    println!("Error queue is empty.");
                } else {
                    for e in errors {
                        println!("{e:?}");
                    }
                }
            }
            ErrorQueueCommand::Delete { index, quiet } => {
                let mut errors = db.queue(Queue::Error)?;
                if !index.is_empty() {
                    errors.retain(|el| index.contains(&el.pk()));
                }
                if errors.is_empty() {
                    if !quiet {
                        println!("Error queue is empty.");
                    }
                } else {
                    if !quiet {
                        println!("Deleting error queue elements {:?}", &index);
                    }
                    db.delete_from_queue(Queue::Error, index)?;
                    if !quiet {
                        for e in errors {
                            println!("{e:?}");
                        }
                    }
                }
            }
        },
        ImportMaildir {
            list_id,
            mut maildir_path,
        } => {
            let list = match list!(db, list_id) {
                Some(v) => v,
                None => {
                    return Err(format!("No list with id or pk {} was found", list_id).into());
                }
            };
            if !maildir_path.is_absolute() {
                maildir_path = std::env::current_dir()
                    .expect("could not detect current directory")
                    .join(&maildir_path);
            }

            fn get_file_hash(file: &std::path::Path) -> EnvelopeHash {
                let mut hasher = DefaultHasher::default();
                file.hash(&mut hasher);
                EnvelopeHash(hasher.finish())
            }
            let mut buf = Vec::with_capacity(4096);
            let files =
                melib::backends::maildir::MaildirType::list_mail_in_maildir_fs(maildir_path, true)?;
            let mut ctr = 0;
            for file in files {
                let hash = get_file_hash(&file);
                let mut reader = std::io::BufReader::new(std::fs::File::open(&file)?);
                buf.clear();
                reader.read_to_end(&mut buf)?;
                if let Ok(mut env) = Envelope::from_bytes(buf.as_slice(), Some(file.flags())) {
                    env.set_hash(hash);
                    db.insert_post(list.pk, &buf, &env)?;
                    ctr += 1;
                }
            }
            println!("Inserted {} posts to {}.", ctr, list_id);
        }
        UpdatePostfixConfig {
            master_cf,
            config:
                PostfixConfig {
                    user,
                    group,
                    binary_path,
                    process_limit,
                    map_output_path,
                    transport_name,
                },
        } => {
            let pfconf = mailpot::postfix::PostfixConfiguration {
                user: user.into(),
                group: group.map(Into::into),
                binary_path,
                process_limit,
                map_output_path,
                transport_name: transport_name.map(std::borrow::Cow::from),
            };
            pfconf.save_maps(db.conf())?;
            pfconf.save_master_cf_entry(db.conf(), config_path, master_cf.as_deref())?;
        }
        PrintPostfixConfig {
            config:
                PostfixConfig {
                    user,
                    group,
                    binary_path,
                    process_limit,
                    map_output_path,
                    transport_name,
                },
        } => {
            let pfconf = mailpot::postfix::PostfixConfiguration {
                user: user.into(),
                group: group.map(Into::into),
                binary_path,
                process_limit,
                map_output_path,
                transport_name: transport_name.map(std::borrow::Cow::from),
            };
            let lists = db.lists()?;
            let lists_post_policies = lists
                .into_iter()
                .map(|l| {
                    let pk = l.pk;
                    Ok((l, db.list_post_policy(pk)?))
                })
                .collect::<Result<Vec<(DbVal<MailingList>, Option<DbVal<PostPolicy>>)>>>()?;
            let maps = pfconf.generate_maps(&lists_post_policies);
            let mastercf = pfconf.generate_master_cf_entry(db.conf(), config_path);

            println!("{maps}\n\n{mastercf}\n");
        }
        Accounts => {
            let accounts = db.accounts()?;
            if accounts.is_empty() {
                println!("No accounts found.");
            } else {
                for a in accounts {
                    println!("- {:?}", a);
                }
            }
        }
        AccountInfo { address } => {
            if let Some(acc) = db.account_by_address(&address)? {
                let subs = db.account_subscriptions(acc.pk())?;
                if subs.is_empty() {
                    println!("No subscriptions found.");
                } else {
                    for s in subs {
                        let list = db
                            .list(s.list)
                            .unwrap_or_else(|err| {
                                panic!(
                                    "Found subscription with list_pk = {} but no such list \
                                     exists.\nListSubscription = {:?}\n\n{err}",
                                    s.list, s
                                )
                            })
                            .unwrap_or_else(|| {
                                panic!(
                                    "Found subscription with list_pk = {} but no such list \
                                     exists.\nListSubscription = {:?}",
                                    s.list, s
                                )
                            });
                        println!("- {:?} {}", s, list);
                    }
                }
            } else {
                println!("account with this address not found!");
            };
        }
        AddAccount {
            address,
            password,
            name,
            public_key,
            enabled,
        } => {
            db.add_account(Account {
                pk: 0,
                name,
                address,
                public_key,
                password,
                enabled: enabled.unwrap_or(true),
            })?;
        }
        RemoveAccount { address } => {
            let mut input = String::new();
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

            db.remove_account(&address)?;
        }
        UpdateAccount {
            address,
            password,
            name,
            public_key,
            enabled,
        } => {
            let changeset = AccountChangeset {
                address,
                name,
                public_key,
                password,
                enabled,
            };
            db.update_account(changeset)?;
        }
        Repair {
            fix,
            all,
            mut datetime_header_value,
            mut remove_empty_accounts,
            mut remove_accepted_subscription_requests,
            mut warn_list_no_owner,
        } => {
            type LintFn =
                fn(&'_ mut mailpot::Connection, bool) -> std::result::Result<(), mailpot::Error>;
            let dry_run = !fix;
            if all {
                datetime_header_value = true;
                remove_empty_accounts = true;
                remove_accepted_subscription_requests = true;
                warn_list_no_owner = true;
            }

            if !(datetime_header_value
                | remove_empty_accounts
                | remove_accepted_subscription_requests
                | warn_list_no_owner)
            {
                return Err(
                    "No lints selected: specify them with flag arguments. See --help".into(),
                );
            }

            if dry_run {
                println!("running without making modifications (dry run)");
            }

            for (flag, lint_fn) in [
                (datetime_header_value, datetime_header_value_lint as LintFn),
                (remove_empty_accounts, remove_empty_accounts_lint as _),
                (
                    remove_accepted_subscription_requests,
                    remove_accepted_subscription_requests_lint as _,
                ),
                (warn_list_no_owner, warn_list_no_owner_lint as _),
            ] {
                if flag {
                    lint_fn(&mut db, dry_run)?;
                }
            }
        }
    }

    Ok(())
}

fn main() -> std::result::Result<(), i32> {
    let opt = Opt::parse();
    stderrlog::new()
        .module(module_path!())
        .module("mailpot")
        .quiet(opt.quiet)
        .verbosity(opt.verbose as usize)
        .timestamp(opt.ts.unwrap_or(stderrlog::Timestamp::Off))
        .init()
        .unwrap();
    if let Err(err) = run_app(opt) {
        print!("{}", err.display_chain());
        std::process::exit(-1);
    }
    Ok(())
}
