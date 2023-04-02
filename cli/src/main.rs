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

pub use mailpot::config::*;
pub use mailpot::db::*;
pub use mailpot::errors::*;
pub use mailpot::mail::*;
pub use mailpot::models::changesets::*;
pub use mailpot::models::*;
pub use mailpot::*;
use std::path::PathBuf;
use structopt::StructOpt;

macro_rules! get_list {
    ($db:ident, $list_id:expr) => {{
        $db.get_list_by_id(&$list_id)?.or_else(|| {
            $list_id
                .parse::<i64>()
                .ok()
                .map(|pk| $db.get_list(pk).ok())
                .flatten()
                .flatten()
        })
    }};
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "mailpot",
    about = "mini mailing list manager",
    author = "Manos Pitsidianakis <epilys@nessuent.xyz>",
    //manpage = "docs/main.mdoc",
    //manpage_header = "docs/header.mdoc",
    //manpage_footer = "docs/footer.mdoc"
)]
struct Opt {
    /// Activate debug mode
    #[structopt(short, long)]
    debug: bool,

    /// Set config file
    #[structopt(short, long, parse(from_os_str))]
    #[allow(dead_code)]
    config: PathBuf,
    #[structopt(flatten)]
    cmd: Command,
    /// Silence all output
    #[structopt(short = "q", long = "quiet")]
    quiet: bool,
    /// Verbose mode (-v, -vv, -vvv, etc)
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: usize,
    /// Timestamp (sec, ms, ns, none)
    #[structopt(short = "t", long = "timestamp")]
    ts: Option<stderrlog::Timestamp>,
}

#[derive(Debug, StructOpt)]
//#[structopt(manpage = "docs/command.mdoc")]
enum Command {
    /// Prints a sample config file to STDOUT
    SampleConfig,
    ///Dumps database data to STDOUT
    DumpDatabase,
    ///Lists all registered mailing lists
    ListLists,
    ///Mailing list management
    List {
        ///Selects mailing list to operate on
        list_id: String,
        #[structopt(subcommand)]
        cmd: ListCommand,
    },
    ///Create new list
    CreateList {
        ///List name
        #[structopt(long)]
        name: String,
        ///List ID
        #[structopt(long)]
        id: String,
        ///List e-mail address
        #[structopt(long)]
        address: String,
        ///List description
        #[structopt(long)]
        description: Option<String>,
        ///List archive URL
        #[structopt(long)]
        archive_url: Option<String>,
    },
    ///Post message from STDIN to list
    Post {
        #[structopt(long)]
        dry_run: bool,
    },
    /// Mail that has not been handled properly end up in the error queue.
    ErrorQueue {
        #[structopt(subcommand)]
        cmd: ErrorQueueCommand,
    },
    /// Import a maildir folder into an existing list.
    ImportMaildir {
        ///Selects mailing list to operate on
        list_id: String,
        #[structopt(long, parse(from_os_str))]
        maildir_path: PathBuf,
    },
}

#[derive(Debug, StructOpt)]
//#[structopt(manpage = "docs/error_queue.mdoc")]
enum ErrorQueueCommand {
    /// List.
    List,
    /// Print entry in RFC5322 or JSON format.
    Print {
        /// index of entry.
        #[structopt(long)]
        index: Vec<i64>,
        /// JSON format.
        #[structopt(long)]
        json: bool,
    },
    /// Delete entry and print it in stdout.
    Delete {
        /// index of entry.
        #[structopt(long)]
        index: Vec<i64>,
        /// Do not print in stdout.
        #[structopt(long)]
        quiet: bool,
    },
}

#[derive(Debug, StructOpt)]
//#[structopt(manpage = "docs/list.mdoc")]
enum ListCommand {
    /// List members of list.
    Members,
    /// Add member to list.
    AddMember {
        /// E-mail address
        #[structopt(long)]
        address: String,
        /// Name
        #[structopt(long)]
        name: Option<String>,
        /// Send messages as digest?
        #[structopt(long)]
        digest: bool,
        /// Hide message from list when posting?
        #[structopt(long)]
        hide_address: bool,
        /// Hide message from list when posting?
        #[structopt(long)]
        /// Receive confirmation email when posting?
        receive_confirmation: Option<bool>,
        #[structopt(long)]
        /// Receive posts from list even if address exists in To or Cc header?
        receive_duplicates: Option<bool>,
        #[structopt(long)]
        /// Receive own posts from list?
        receive_own_posts: Option<bool>,
        #[structopt(long)]
        /// Is subscription enabled?
        enabled: Option<bool>,
    },
    /// Remove member from list.
    RemoveMember {
        #[structopt(long)]
        /// E-mail address
        address: String,
    },
    /// Update membership info.
    UpdateMembership {
        address: String,
        name: Option<String>,
        digest: Option<bool>,
        hide_address: Option<bool>,
        receive_duplicates: Option<bool>,
        receive_own_posts: Option<bool>,
        receive_confirmation: Option<bool>,
        enabled: Option<bool>,
    },
    /// Add policy to list.
    AddPolicy {
        #[structopt(long)]
        announce_only: bool,
        #[structopt(long)]
        subscriber_only: bool,
        #[structopt(long)]
        approval_needed: bool,
        #[structopt(long)]
        no_subscriptions: bool,
        #[structopt(long)]
        custom: bool,
    },
    RemovePolicy {
        #[structopt(long)]
        pk: i64,
    },
    /// Add list owner to list.
    AddListOwner {
        #[structopt(long)]
        address: String,
        #[structopt(long)]
        name: Option<String>,
    },
    RemoveListOwner {
        #[structopt(long)]
        pk: i64,
    },
    /// Alias for update-membership --enabled true
    EnableMembership { address: String },
    /// Alias for update-membership --enabled false
    DisableMembership { address: String },
    /// Update mailing list details.
    Update {
        name: Option<String>,
        id: Option<String>,
        address: Option<String>,
        description: Option<String>,
        archive_url: Option<String>,
    },
    /// Show mailing list health status.
    Health,
    /// Show mailing list info.
    Info,
}

fn run_app(opt: Opt) -> Result<()> {
    if opt.debug {
        println!("DEBUG: {:?}", &opt);
    }
    if let Command::SampleConfig = opt.cmd {
        println!("{}", Configuration::new("/path/to/sqlite.db").to_toml());
        return Ok(());
    };
    let config = Configuration::from_file(opt.config.as_path())?;
    use Command::*;
    let mut db = Database::open_or_create_db(&config)?;
    match opt.cmd {
        SampleConfig => {}
        DumpDatabase => {
            let lists = db.list_lists()?;
            let mut stdout = std::io::stdout();
            serde_json::to_writer_pretty(&mut stdout, &lists)?;
            for l in &lists {
                serde_json::to_writer_pretty(&mut stdout, &db.list_members(l.pk)?)?;
            }
        }
        ListLists => {
            let lists = db.list_lists()?;
            if lists.is_empty() {
                println!("No lists found.");
            } else {
                for l in lists {
                    println!("- {} {:?}", l.id, l);
                    let list_owners = db.get_list_owners(l.pk)?;
                    if list_owners.is_empty() {
                        println!("\tList owners: None");
                    } else {
                        println!("\tList owners:");
                        for o in list_owners {
                            println!("\t- {}", o);
                        }
                    }
                    if let Some(s) = db.get_list_policy(l.pk)? {
                        println!("\tList policy: {}", s);
                    } else {
                        println!("\tList policy: None");
                    }
                    println!();
                }
            }
        }
        List { list_id, cmd } => {
            let list = match get_list!(db, list_id) {
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
                    name,
                    digest,
                    hide_address,
                    receive_confirmation,
                    receive_duplicates,
                    receive_own_posts,
                    enabled,
                } => {
                    db.add_member(
                        list.pk,
                        ListMembership {
                            pk: 0,
                            list: list.pk,
                            name,
                            address,
                            digest,
                            hide_address,
                            receive_confirmation: receive_confirmation.unwrap_or(true),
                            receive_duplicates: receive_duplicates.unwrap_or(true),
                            receive_own_posts: receive_own_posts.unwrap_or(false),
                            enabled: enabled.unwrap_or(true),
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

                    db.remove_member(list.pk, &address)?;
                }
                Health => {
                    println!("{} health:", list);
                    let list_owners = db.get_list_owners(list.pk)?;
                    let list_policy = db.get_list_policy(list.pk)?;
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
                    let list_owners = db.get_list_owners(list.pk)?;
                    let list_policy = db.get_list_policy(list.pk)?;
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
                    name,
                    digest,
                    hide_address,
                    receive_duplicates,
                    receive_own_posts,
                    receive_confirmation,
                    enabled,
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
                    no_subscriptions,
                    custom,
                } => {
                    let policy = PostPolicy {
                        pk: 0,
                        list: list.pk,
                        announce_only,
                        subscriber_only,
                        approval_needed,
                        no_subscriptions,
                        custom,
                    };
                    let new_val = db.set_list_policy(list.pk, policy)?;
                    println!("Added new policy with pk = {}", new_val.pk());
                }
                RemovePolicy { pk } => {
                    db.remove_list_policy(list.pk, pk)?;
                    println!("Removed policy with pk = {}", pk);
                }
                AddListOwner { address, name } => {
                    let list_owner = ListOwner {
                        pk: 0,
                        list: list.pk,
                        address,
                        name,
                    };
                    let new_val = db.add_list_owner(list.pk, list_owner)?;
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
                        hide_address: None,
                        receive_duplicates: None,
                        receive_own_posts: None,
                        receive_confirmation: None,
                        enabled: Some(true),
                    };
                    db.update_member(changeset)?;
                }
                DisableMembership { address } => {
                    let changeset = ListMembershipChangeset {
                        list: list.pk,
                        address,
                        name: None,
                        digest: None,
                        hide_address: None,
                        receive_duplicates: None,
                        receive_own_posts: None,
                        receive_confirmation: None,
                        enabled: Some(false),
                    };
                    db.update_member(changeset)?;
                }
                Update {
                    name,
                    id,
                    address,
                    description,
                    archive_url,
                } => {
                    let description = if description
                        .as_ref()
                        .map(|s: &String| s.is_empty())
                        .unwrap_or(false)
                    {
                        None
                    } else {
                        Some(description)
                    };
                    let archive_url = if archive_url
                        .as_ref()
                        .map(|s: &String| s.is_empty())
                        .unwrap_or(false)
                    {
                        None
                    } else {
                        Some(archive_url)
                    };
                    let changeset = MailingListChangeset {
                        pk: list.pk,
                        name,
                        id,
                        address,
                        description,
                        archive_url,
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
                        std::dbg!(&env);
                    }
                    db.post(&env, input.as_bytes(), dry_run)?;
                }
                Err(err) if input.trim().is_empty() => {
                    eprintln!("Empty input, abort.");
                    return Err(err.into());
                }
                Err(err) => {
                    eprintln!("Could not parse message: {}", err);
                    let p = config.save_message(input)?;
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
            let list = match get_list!(db, list_id) {
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
    }

    Ok(())
}

fn main() -> std::result::Result<(), i32> {
    let opt = Opt::from_args();
    stderrlog::new()
        .module(module_path!())
        .module("mailpot")
        .quiet(opt.quiet)
        .verbosity(opt.verbose)
        .timestamp(opt.ts.unwrap_or(stderrlog::Timestamp::Off))
        .init()
        .unwrap();
    if let Err(err) = run_app(opt) {
        println!("{}", err.display_chain());
        std::process::exit(-1);
    }
    Ok(())
}
