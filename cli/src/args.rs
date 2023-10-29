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

pub use std::path::PathBuf;

pub use clap::{builder::TypedValueParser, Args, CommandFactory, Parser, Subcommand};

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
    #[arg(short, long, value_parser)]
    pub config: Option<PathBuf>,
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
    /// You can generate a new configuration file by writing the output to a
    /// file, e.g: mpot sample-config --with-smtp > config.toml
    SampleConfig {
        /// Use an SMTP connection instead of a shell process.
        #[arg(long)]
        with_smtp: bool,
    },
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
    /// Flush outgoing e-mail queue.
    FlushQueue {
        /// Show e-mail processing result without actually consuming it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Mail that has not been handled properly end up in the error queue.
    ErrorQueue {
        #[command(subcommand)]
        cmd: QueueCommand,
    },
    /// Mail that has not been handled properly end up in the error queue.
    Queue {
        #[arg(long, value_parser = QueueValueParser)]
        queue: mailpot::queue::Queue,
        #[command(subcommand)]
        cmd: QueueCommand,
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
        /// Override location of master.cf file (default:
        /// /etc/postfix/master.cf)
        master_cf: Option<PathBuf>,
        #[clap(flatten)]
        config: PostfixConfig,
    },
    /// Print postfix maps and master.cf entry to STDOUT.
    ///
    /// Map output should be added to transport_maps and local_recipient_maps
    /// parameters in postfix's main.cf. It must be saved in a plain text
    /// file. To make postfix be able to read them, the postmap application
    /// must be executed with the path to the map file as its sole argument.
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
    /// All Accounts.
    Accounts,
    /// Account info.
    AccountInfo {
        /// Account address.
        address: String,
    },
    /// Add account.
    AddAccount {
        /// E-mail address.
        #[arg(long)]
        address: String,
        /// SSH public key for authentication.
        #[arg(long)]
        password: String,
        /// Name.
        #[arg(long)]
        name: Option<String>,
        /// Public key.
        #[arg(long)]
        public_key: Option<String>,
        #[arg(long)]
        /// Is account enabled.
        enabled: Option<bool>,
    },
    /// Remove account.
    RemoveAccount {
        #[arg(long)]
        /// E-mail address.
        address: String,
    },
    /// Update account info.
    UpdateAccount {
        /// Address to edit.
        address: String,
        /// Public key for authentication.
        #[arg(long)]
        password: Option<String>,
        /// Name.
        #[arg(long)]
        name: Option<Option<String>>,
        /// Public key.
        #[arg(long)]
        public_key: Option<Option<String>>,
        #[arg(long)]
        /// Is account enabled.
        enabled: Option<Option<bool>>,
    },
    /// Show and fix possible data mistakes or inconsistencies.
    Repair {
        /// Fix errors (default: false)
        #[arg(long, default_value = "false")]
        fix: bool,
        /// Select all tests (default: false)
        #[arg(long, default_value = "false")]
        all: bool,
        /// Post `datetime` column must have the Date: header value, in RFC2822
        /// format.
        #[arg(long, default_value = "false")]
        datetime_header_value: bool,
        /// Remove accounts that have no matching subscriptions.
        #[arg(long, default_value = "false")]
        remove_empty_accounts: bool,
        /// Remove subscription requests that have been accepted.
        #[arg(long, default_value = "false")]
        remove_accepted_subscription_requests: bool,
        /// Warn if a list has no owners.
        #[arg(long, default_value = "false")]
        warn_list_no_owner: bool,
    },
}

/// Postfix config values.
#[derive(Debug, Args)]
pub struct PostfixConfig {
    /// User that runs mailpot when postfix relays a message.
    ///
    /// Must not be the `postfix` user.
    /// Must have permissions to access the database file and the data
    /// directory.
    #[arg(short, long)]
    pub user: String,
    /// Group that runs mailpot when postfix relays a message.
    /// Optional.
    #[arg(short, long)]
    pub group: Option<String>,
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
    /// Each entry in the `master.cf` configuration file defines a single
    /// service.
    ///
    /// The `master.cf` file is documented in [`master(5)`](https://www.postfix.org/master.5.html):
    /// <https://www.postfix.org/master.5.html>.
    #[arg(long)]
    pub transport_name: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum QueueCommand {
    /// List.
    List,
    /// Print entry in RFC5322 or JSON format.
    Print {
        /// index of entry.
        #[arg(long)]
        index: Vec<i64>,
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

/// Subscription options.
#[derive(Debug, Args)]
pub struct SubscriptionOptions {
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

/// Account options.
#[derive(Debug, Args)]
pub struct AccountOptions {
    /// Name.
    #[arg(long)]
    pub name: Option<String>,
    /// Public key.
    #[arg(long)]
    pub public_key: Option<String>,
    #[arg(long)]
    /// Is account enabled.
    pub enabled: Option<bool>,
}

#[derive(Debug, Subcommand)]
pub enum ListCommand {
    /// List subscriptions of list.
    Subscriptions,
    /// List subscription requests.
    SubscriptionRequests,
    /// Add subscription to list.
    AddSubscription {
        /// E-mail address.
        #[arg(long)]
        address: String,
        #[clap(flatten)]
        subscription_options: SubscriptionOptions,
    },
    /// Remove subscription from list.
    RemoveSubscription {
        #[arg(long)]
        /// E-mail address.
        address: String,
    },
    /// Update subscription info.
    UpdateSubscription {
        /// Address to edit.
        address: String,
        #[clap(flatten)]
        subscription_options: SubscriptionOptions,
    },
    /// Accept a subscription request by its primary key.
    AcceptSubscriptionRequest {
        /// The primary key of the request.
        pk: i64,
    },
    /// Add a new post policy.
    AddPostPolicy {
        #[arg(long)]
        /// Only list owners can post.
        announce_only: bool,
        #[arg(long)]
        /// Only subscriptions can post.
        subscription_only: bool,
        #[arg(long)]
        /// Subscriptions can post.
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
    RemovePostPolicy {
        #[arg(long)]
        /// Post policy primary key.
        pk: i64,
    },
    /// Add subscription policy to list.
    AddSubscriptionPolicy {
        #[arg(long)]
        /// Send confirmation e-mail when subscription is finalized.
        send_confirmation: bool,
        #[arg(long)]
        /// Anyone can subscribe without restrictions.
        open: bool,
        #[arg(long)]
        /// Only list owners can manually add subscriptions.
        manual: bool,
        #[arg(long)]
        /// Anyone can request to subscribe.
        request: bool,
        #[arg(long)]
        /// Allow subscriptions, but handle it manually.
        custom: bool,
    },
    RemoveSubscriptionPolicy {
        #[arg(long)]
        /// Subscription policy primary key.
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
    /// Alias for update-subscription --enabled true.
    EnableSubscription {
        /// Subscription address.
        address: String,
    },
    /// Alias for update-subscription --enabled false.
    DisableSubscription {
        /// Subscription address.
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
        /// Subscriptions that are initiated from the subscription's address are
        /// verified automatically.
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
        /// If not enabled, the list will continue to show up in the database
        /// but e-mails and requests to it won't work.
        #[arg(long)]
        enabled: Option<bool>,
    },
    /// Show mailing list health status.
    Health,
    /// Show mailing list info.
    Info,
    /// Import members in a local list from a remote mailman3 REST API instance.
    ///
    /// To find the id of the remote list, you can check URL/lists.
    /// Example with curl:
    ///
    /// curl --anyauth -u admin:pass "http://localhost:9001/3.0/lists"
    ///
    /// If you're trying to import an entire list, create it first and then
    /// import its users with this command.
    ///
    /// Example:
    /// mpot -c conf.toml list list-general import-members --url "http://localhost:9001/3.0/" --username admin --password password --list-id list-general.example.com --skip-owners --dry-run
    ImportMembers {
        #[arg(long)]
        /// REST HTTP endpoint e.g. http://localhost:9001/3.0/
        url: String,
        #[arg(long)]
        /// REST HTTP Basic Authentication username.
        username: String,
        #[arg(long)]
        /// REST HTTP Basic Authentication password.
        password: String,
        #[arg(long)]
        /// List ID of remote list to query.
        list_id: String,
        /// Show what would be inserted without performing any changes.
        #[arg(long)]
        dry_run: bool,
        /// Don't import list owners.
        #[arg(long)]
        skip_owners: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct QueueValueParser;

impl QueueValueParser {
    pub fn new() -> Self {
        Self
    }
}

impl TypedValueParser for QueueValueParser {
    type Value = mailpot::queue::Queue;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> std::result::Result<Self::Value, clap::Error> {
        TypedValueParser::parse(self, cmd, arg, value.to_owned())
    }

    fn parse(
        &self,
        cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: std::ffi::OsString,
    ) -> std::result::Result<Self::Value, clap::Error> {
        use std::str::FromStr;

        use clap::error::ErrorKind;

        if value.is_empty() {
            return Err(cmd.clone().error(
                ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
                "queue value required",
            ));
        }
        Self::Value::from_str(value.to_str().ok_or_else(|| {
            cmd.clone().error(
                ErrorKind::InvalidValue,
                "Queue value is not an UTF-8 string",
            )
        })?)
        .map_err(|err| cmd.clone().error(ErrorKind::InvalidValue, err))
    }
}

impl Default for QueueValueParser {
    fn default() -> Self {
        Self::new()
    }
}
