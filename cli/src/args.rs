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

extern crate stderrlog;

use std::path::PathBuf;

pub use clap::{Args, CommandFactory, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "mpot",
    about = "mailing list manager",
    long_about = "Tool for mailpot mailing list management.",
    before_long_help = "GNU Affero version 3 or later <https://www.gnu.org/licenses/>",
    author,
    version
)]
pub struct Opt {
    /// Print logs.
    #[arg(short, long)]
    pub debug: bool,

    /// Configuration file to use.
    #[arg(short, long, required = true, value_parser)]
    pub config: PathBuf,
    #[command(subcommand)]
    pub cmd: Command,
    /// Silence all output.
    #[arg(short, long)]
    pub quiet: bool,
    /// Verbose mode (-v, -vv, -vvv, etc).
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// Debug log timestamp (sec, ms, ns, none).
    #[arg(short, long)]
    pub ts: Option<stderrlog::Timestamp>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Prints a sample config file to STDOUT.
    ///
    /// You can generate a new configuration file by writing the output to a file, e.g:
    /// mpot sample-config > config.toml
    SampleConfig,
    /// Dumps database data to STDOUT.
    DumpDatabase,
    /// Lists all registered mailing lists.
    ListLists,
    /// Mailing list management.
    List {
        /// Selects mailing list to operate on.
        list_id: String,
        #[command(subcommand)]
        cmd: ListCommand,
    },
    /// Create new list.
    CreateList {
        /// List name.
        #[arg(long)]
        name: String,
        /// List ID.
        #[arg(long)]
        id: String,
        /// List e-mail address.
        #[arg(long)]
        address: String,
        /// List description.
        #[arg(long)]
        description: Option<String>,
        /// List archive URL.
        #[arg(long)]
        archive_url: Option<String>,
    },
    /// Post message from STDIN to list.
    Post {
        /// Show e-mail processing result without actually consuming it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Mail that has not been handled properly end up in the error queue.
    ErrorQueue {
        #[command(subcommand)]
        cmd: ErrorQueueCommand,
    },
    /// Import a maildir folder into an existing list.
    ImportMaildir {
        /// List-ID or primary key value.
        list_id: String,
        /// Path to a maildir mailbox.
        /// Must contain {cur, tmp, new} folders.
        #[arg(long, value_parser)]
        maildir_path: PathBuf,
    },
    /// Update postfix maps and master.cf (probably needs root permissions).
    UpdatePostfixConfig {
        #[arg(short = 'p', long)]
        /// Override location of master.cf file (default: /etc/postfix/master.cf)
        master_cf: Option<PathBuf>,
        #[clap(flatten)]
        config: PostfixConfig,
    },
    /// Print postfix maps and master.cf entry to STDOUT.
    ///
    /// Map output should be added to transport_maps and local_recipient_maps parameters in postfix's main.cf.
    /// It must be saved in a plain text file.
    /// To make postfix be able to read them, the postmap application must be executed with the
    /// path to the map file as its sole argument.
    ///
    ///   postmap /path/to/mylist_maps
    ///
    /// postmap is usually distributed along with the other postfix binaries.
    ///
    /// The master.cf entry must be manually appended to the master.cf file. See <https://www.postfix.org/master.5.html>.
    PrintPostfixConfig {
        #[clap(flatten)]
        config: PostfixConfig,
    },
}

/// Postfix config values.
#[derive(Debug, Args)]
pub struct PostfixConfig {
    /// User that runs mailpot when postfix relays a message.
    ///
    /// Must not be the `postfix` user.
    /// Must have permissions to access the database file and the data directory.
    #[arg(short, long)]
    pub user: String,
    /// The path to the mailpot binary postfix will execute.
    #[arg(long)]
    pub binary_path: PathBuf,
    /// Limit the number of mailpot instances that can exist at the same time.
    ///
    /// Default is 1.
    #[arg(long, default_value = "1")]
    pub process_limit: Option<u64>,
    /// The directory in which the map files are saved.
    ///
    /// Default is `data_path` from [`Configuration`](mailpot::Configuration).
    #[arg(long)]
    pub map_output_path: Option<PathBuf>,
    /// The name of the postfix service name to use.
    /// Default is `mailpot`.
    ///
    /// A postfix service is a daemon managed by the postfix process.
    /// Each entry in the `master.cf` configuration file defines a single service.
    ///
    /// The `master.cf` file is documented in [`master(5)`](https://www.postfix.org/master.5.html):
    /// <https://www.postfix.org/master.5.html>.
    #[arg(long)]
    pub transport_name: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ErrorQueueCommand {
    /// List.
    List,
    /// Print entry in RFC5322 or JSON format.
    Print {
        /// index of entry.
        #[arg(long)]
        index: Vec<i64>,
        /// JSON format.
        #[arg(long)]
        json: bool,
    },
    /// Delete entry and print it in stdout.
    Delete {
        /// index of entry.
        #[arg(long)]
        index: Vec<i64>,
        /// Do not print in stdout.
        #[arg(long)]
        quiet: bool,
    },
}

/// Member options.
#[derive(Debug, Args)]
pub struct MemberOptions {
    /// Name.
    #[arg(long)]
    pub name: Option<String>,
    /// Send messages as digest.
    #[arg(long, default_value = "false")]
    pub digest: Option<bool>,
    /// Hide message from list when posting.
    #[arg(long, default_value = "false")]
    pub hide_address: Option<bool>,
    /// Hide message from list when posting.
    #[arg(long, default_value = "false")]
    /// E-mail address verification status.
    pub verified: Option<bool>,
    #[arg(long, default_value = "true")]
    /// Receive confirmation email when posting.
    pub receive_confirmation: Option<bool>,
    #[arg(long, default_value = "true")]
    /// Receive posts from list even if address exists in To or Cc header.
    pub receive_duplicates: Option<bool>,
    #[arg(long, default_value = "false")]
    /// Receive own posts from list.
    pub receive_own_posts: Option<bool>,
    #[arg(long, default_value = "true")]
    /// Is subscription enabled.
    pub enabled: Option<bool>,
}

#[derive(Debug, Subcommand)]
pub enum ListCommand {
    /// List members of list.
    Members,
    /// Add member to list.
    AddMember {
        /// E-mail address.
        #[arg(long)]
        address: String,
        #[clap(flatten)]
        member_options: MemberOptions,
    },
    /// Remove member from list.
    RemoveMember {
        #[arg(long)]
        /// E-mail address.
        address: String,
    },
    /// Update membership info.
    UpdateMembership {
        /// Address to edit.
        address: String,
        #[clap(flatten)]
        member_options: MemberOptions,
    },
    /// Add a new post policy.
    AddPolicy {
        #[arg(long)]
        /// Only list owners can post.
        announce_only: bool,
        #[arg(long)]
        /// Only subscribers can post.
        subscriber_only: bool,
        #[arg(long)]
        /// Subscribers can post.
        /// Other posts must be approved by list owners.
        approval_needed: bool,
        #[arg(long)]
        /// Anyone can post without restrictions.
        open: bool,
        #[arg(long)]
        /// Allow posts, but handle it manually.
        custom: bool,
    },
    // Remove post policy.
    RemovePolicy {
        #[arg(long)]
        /// Post policy primary key.
        pk: i64,
    },
    /// Add subscription policy to list.
    AddSubscribePolicy {
        #[arg(long)]
        /// Send confirmation e-mail when subscription is finalized.
        send_confirmation: bool,
        #[arg(long)]
        /// Anyone can subscribe without restrictions.
        open: bool,
        #[arg(long)]
        /// Only list owners can manually add subscribers.
        manual: bool,
        #[arg(long)]
        /// Anyone can request to subscribe.
        request: bool,
        #[arg(long)]
        /// Allow subscriptions, but handle it manually.
        custom: bool,
    },
    RemoveSubscribePolicy {
        #[arg(long)]
        /// Subscribe policy primary key.
        pk: i64,
    },
    /// Add list owner to list.
    AddListOwner {
        #[arg(long)]
        address: String,
        #[arg(long)]
        name: Option<String>,
    },
    RemoveListOwner {
        #[arg(long)]
        /// List owner primary key.
        pk: i64,
    },
    /// Alias for update-membership --enabled true.
    EnableMembership {
        /// Member address.
        address: String,
    },
    /// Alias for update-membership --enabled false.
    DisableMembership {
        /// Member address.
        address: String,
    },
    /// Update mailing list details.
    Update {
        /// New list name.
        #[arg(long)]
        name: Option<String>,
        /// New List-ID.
        #[arg(long)]
        id: Option<String>,
        /// New list address.
        #[arg(long)]
        address: Option<String>,
        /// New list description.
        #[arg(long)]
        description: Option<String>,
        /// New list archive URL.
        #[arg(long)]
        archive_url: Option<String>,
        /// New owner address local part.
        /// If empty, it defaults to '+owner'.
        #[arg(long)]
        owner_local_part: Option<String>,
        /// New request address local part.
        /// If empty, it defaults to '+request'.
        #[arg(long)]
        request_local_part: Option<String>,
        /// Require verification of e-mails for new subscriptions.
        ///
        /// Subscriptions that are initiated from the member's address are verified automatically.
        #[arg(long)]
        verify: Option<bool>,
        /// Public visibility of list.
        ///
        /// If hidden, the list will not show up in public APIs unless
        /// requests to it won't work.
        #[arg(long)]
        hidden: Option<bool>,
        /// Enable or disable the list's functionality.
        ///
        /// If not enabled, the list will continue to show up in the database but e-mails and
        /// requests to it won't work.
        #[arg(long)]
        enabled: Option<bool>,
    },
    /// Show mailing list health status.
    Health,
    /// Show mailing list info.
    Info,
}
