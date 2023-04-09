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

extern crate log;
extern crate mailpot;
extern crate stderrlog;

pub use mailpot::mail::*;
pub use mailpot::models::changesets::*;
pub use mailpot::models::*;
pub use mailpot::*;

mod args;
use args::*;

macro_rules! list {
    ($db:ident, $list_id:expr) => {{
        $db.list_by_id(&$list_id)?.or_else(|| {
            $list_id
                .parse::<i64>()
                .ok()
                .map(|pk| $db.list(pk).ok())
                .flatten()
        })
    }};
}
fn run_app(opt: Opt) -> Result<()> {
    if opt.debug {
        println!("DEBUG: {:?}", &opt);
    }
    if let Command::SampleConfig = opt.cmd {
        println!("{}", Configuration::new("/path/to/sqlite.db").to_toml());
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
        SampleConfig => {}
        DumpDatabase => {
            let lists = db.lists()?;
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &lists)?;
            for l in &lists {
                serde_json::to_writer_pretty(&mut stdout, &db.list_members(l.pk)?)?;
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
                    if let Some(s) = db.list_policy(l.pk)? {
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
                Members => {
                    let members = db.list_members(list.pk)?;
                    if members.is_empty() {
                        println!("No members found.");
                    } else {
                        println!("Members of list {}", list.id);
                        for l in members {
                            println!("- {}", &l);
                        }
                    }
                }
                AddMember {
                    address,
                    member_options:
                        MemberOptions {
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
                    db.add_member(
                        list.pk,
                        ListMembership {
                            pk: 0,
                            list: list.pk,
                            name,
                            address,
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
                RemoveMember { address } => {
                    loop {
                        println!(
                            "Are you sure you want to remove membership of {} from list {}? [Yy/n]",
                            address, list
                        );
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input)?;
                        if input.trim() == "Y" || input.trim() == "y" || input.trim() == "" {
                            break;
                        } else if input.trim() == "n" {
                            return Ok(());
                        }
                    }

                    db.remove_membership(list.pk, &address)?;
                }
                Health => {
                    println!("{} health:", list);
                    let list_owners = db.list_owners(list.pk)?;
                    let list_policy = db.list_policy(list.pk)?;
                    if list_owners.is_empty() {
                        println!("\tList has no owners: you should add at least one.");
                    } else {
                        for owner in list_owners {
                            println!("\tList owner: {}.", owner);
                        }
                    }
                    if let Some(list_policy) = list_policy {
                        println!("\tList has post policy: {}.", list_policy);
                    } else {
                        println!("\tList has no post policy: you should add one.");
                    }
                }
                Info => {
                    println!("{} info:", list);
                    let list_owners = db.list_owners(list.pk)?;
                    let list_policy = db.list_policy(list.pk)?;
                    let members = db.list_members(list.pk)?;
                    if members.is_empty() {
                        println!("No members.");
                    } else if members.len() == 1 {
                        println!("1 member.");
                    } else {
                        println!("{} members.", members.len());
                    }
                    if list_owners.is_empty() {
                        println!("List owners: None");
                    } else {
                        println!("List owners:");
                        for o in list_owners {
                            println!("\t- {}", o);
                        }
                    }
                    if let Some(s) = list_policy {
                        println!("List policy: {}", s);
                    } else {
                        println!("List policy: None");
                    }
                }
                UpdateMembership {
                    address,
                    member_options:
                        MemberOptions {
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
                    let changeset = ListMembershipChangeset {
                        list: list.pk,
                        address,
                        name,
                        digest,
                        verified,
                        hide_address,
                        receive_duplicates,
                        receive_own_posts,
                        receive_confirmation,
                        enabled,
                    };
                    db.update_member(changeset)?;
                }
                AddPolicy {
                    announce_only,
                    subscriber_only,
                    approval_needed,
                    open,
                    custom,
                } => {
                    let policy = PostPolicy {
                        pk: 0,
                        list: list.pk,
                        announce_only,
                        subscriber_only,
                        approval_needed,
                        open,
                        custom,
                    };
                    let new_val = db.set_list_policy(policy)?;
                    println!("Added new policy with pk = {}", new_val.pk());
                }
                RemovePolicy { pk } => {
                    db.remove_list_policy(list.pk, pk)?;
                    println!("Removed policy with pk = {}", pk);
                }
                AddSubscribePolicy {
                    send_confirmation,
                    open,
                    manual,
                    request,
                    custom,
                } => {
                    let policy = SubscribePolicy {
                        pk: 0,
                        list: list.pk,
                        send_confirmation,
                        open,
                        manual,
                        request,
                        custom,
                    };
                    let new_val = db.set_list_subscribe_policy(policy)?;
                    println!("Added new subscribe policy with pk = {}", new_val.pk());
                }
                RemoveSubscribePolicy { pk } => {
                    db.remove_list_subscribe_policy(list.pk, pk)?;
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
                EnableMembership { address } => {
                    let changeset = ListMembershipChangeset {
                        list: list.pk,
                        address,
                        name: None,
                        digest: None,
                        verified: None,
                        enabled: Some(true),
                        hide_address: None,
                        receive_duplicates: None,
                        receive_own_posts: None,
                        receive_confirmation: None,
                    };
                    db.update_member(changeset)?;
                }
                DisableMembership { address } => {
                    let changeset = ListMembershipChangeset {
                        list: list.pk,
                        address,
                        name: None,
                        digest: None,
                        enabled: Some(false),
                        verified: None,
                        hide_address: None,
                        receive_duplicates: None,
                        receive_own_posts: None,
                        receive_confirmation: None,
                    };
                    db.update_member(changeset)?;
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
                    macro_rules! string_opts {
                        ($field:ident) => {
                            if $field
                                .as_ref()
                                .map(|s: &String| s.is_empty())
                                .unwrap_or(false)
                            {
                                None
                            } else {
                                Some($field)
                            }
                        };
                    }
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

            use melib::Envelope;
            use std::io::Read;

            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            match Envelope::from_bytes(input.as_bytes(), None) {
                Ok(env) => {
                    if opt.debug {
                        eprintln!("{:?}", &env);
                    }
                    db.post(&env, input.as_bytes(), dry_run)?;
                }
                Err(err) if input.trim().is_empty() => {
                    eprintln!("Empty input, abort.");
                    return Err(err.into());
                }
                Err(err) => {
                    eprintln!("Could not parse message: {}", err);
                    let p = db.conf().save_message(input)?;
                    eprintln!("Message saved at {}", p.display());
                    return Err(err.into());
                }
            }
        }
        ErrorQueue { cmd } => match cmd {
            ErrorQueueCommand::List => {
                let errors = db.error_queue()?;
                if errors.is_empty() {
                    println!("Error queue is empty.");
                } else {
                    for e in errors {
                        println!(
                            "- {} {} {} {} {}",
                            e["pk"],
                            e["datetime"],
                            e["from_address"],
                            e["to_address"],
                            e["subject"]
                        );
                    }
                }
            }
            ErrorQueueCommand::Print { index, json } => {
                let mut errors = db.error_queue()?;
                if !index.is_empty() {
                    errors.retain(|el| index.contains(&el.pk()));
                }
                if errors.is_empty() {
                    println!("Error queue is empty.");
                } else {
                    for e in errors {
                        if json {
                            println!("{:#}", e);
                        } else {
                            println!("{}", e["message"]);
                        }
                    }
                }
            }
            ErrorQueueCommand::Delete { index, quiet } => {
                let mut errors = db.error_queue()?;
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
                    db.delete_from_error_queue(index)?;
                    if !quiet {
                        for e in errors {
                            println!("{}", e["message"]);
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
            use melib::backends::maildir::MaildirPathTrait;
            use melib::{Envelope, EnvelopeHash};
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            use std::io::Read;

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
                    binary_path,
                    process_limit,
                    map_output_path,
                    transport_name,
                },
        } => {
            let pfconf = mailpot::postfix::PostfixConfiguration {
                user: user.into(),
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
                    binary_path,
                    process_limit,
                    map_output_path,
                    transport_name,
                },
        } => {
            let pfconf = mailpot::postfix::PostfixConfiguration {
                user: user.into(),
                binary_path,
                process_limit,
                map_output_path,
                transport_name: transport_name.map(std::borrow::Cow::from),
            };
            let lists = db.lists()?;
            let lists_policies = lists
                .into_iter()
                .map(|l| {
                    let pk = l.pk;
                    Ok((l, db.list_policy(pk)?))
                })
                .collect::<Result<Vec<(DbVal<MailingList>, Option<DbVal<PostPolicy>>)>>>()?;
            let maps = pfconf.generate_maps(&lists_policies);
            let mastercf = pfconf.generate_master_cf_entry(db.conf(), config_path);

            println!("{maps}\n\n{mastercf}\n");
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
        println!("{}", err.display_chain());
        std::process::exit(-1);
    }
    Ok(())
}
