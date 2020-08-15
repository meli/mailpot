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

// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate serde;
use structopt::StructOpt;

pub mod config;
pub use config::*;
pub mod models;
pub mod post;
pub use models::*;
pub mod errors;
pub use errors::*;
pub mod db;
pub use db::*;

use std::path::PathBuf;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "mailpot",
    about = "mini mailing list manager",
    author = "Manos Pitsidianakis <epilys@nessuent.xyz>"
)]
struct Opt {
    /// Activate debug mode
    #[structopt(short, long)]
    debug: bool,

    /// Set config file
    #[structopt(short, long, parse(from_os_str))]
    config: Option<PathBuf>,
    #[structopt(flatten)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    ///Prints database filesystem location
    DbLocation,
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
}

#[derive(Debug, StructOpt)]
enum ListCommand {
    Members,
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
        #[structopt(long)]
        receive_confirmation: Option<bool>,
        #[structopt(long)]
        receive_duplicates: Option<bool>,
        #[structopt(long)]
        receive_own_posts: Option<bool>,
    },
    RemoveMember {
        #[structopt(long)]
        /// E-mail address
        address: String,
    },
    Health,
}

fn run_app(opt: Opt) -> Result<()> {
    if opt.debug {
        println!("DEBUG: {:?}", &opt);
    }
    use Command::*;
    match opt.cmd {
        DbLocation => {
            println!("{}", Database::db_path()?.display());
        }
        ListLists => {
            let db = Database::open_or_create_db()?;
            let lists = db.list_lists()?;
            if lists.is_empty() {
                println!("No lists found.");
            } else {
                for l in lists {
                    println!("- {} {:?}", l.id, l);
                }
            }
        }
        List { list_id, cmd } => {
            let db = Database::open_or_create_db()?;
            let mut lists = db.list_lists()?;
            let list = if let Some(pos) = lists.iter().position(|l| l.id == list_id) {
                lists.remove(pos)
            } else {
                return Err(format!("No list with id {} was found", list_id))?;
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
                } => {
                    db.add_member(
                        list.pk,
                        ListMembership {
                            list: list.pk,
                            name,
                            address,
                            digest,
                            hide_address,
                            receive_confirmation: receive_confirmation.unwrap_or(true),
                            receive_duplicates: receive_duplicates.unwrap_or(true),
                            receive_own_posts: receive_own_posts.unwrap_or(false),
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
            let db = Database::open_or_create_db()?;
            db.create_list(MailingList {
                pk: 0,
                name,
                id,
                description,
                address,
                archive_url,
            })?;
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
                    let db = Database::open_or_create_db()?;
                    db.post(env, input.as_bytes())?;
                }
                Err(err) => {
                    eprintln!("Could not parse message: {}", err);
                    let p = Configuration::save_message(input)?;
                    eprintln!("Message saved at {}", p.display());
                    Err(err)?;
                }
            }
        }
    }

    Ok(())
}

fn main() -> std::result::Result<(), i32> {
    let opt = Opt::from_args();
    if let Err(err) = run_app(opt) {
        println!("{}", err);
        std::process::exit(-1);
    }
    Ok(())
}
