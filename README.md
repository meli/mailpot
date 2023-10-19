# mailpot - mailing list manager

[![Latest Version]][crates.io]&nbsp;[![Coverage]][grcov-rport]&nbsp;[![docs.rs]][rustdoc]&nbsp;![Top Language]&nbsp;![License]

[Latest Version]: https://img.shields.io/crates/v/mailpot.svg?color=white
[crates.io]: https://crates.io/crates/mailpot
[Top Language]: https://img.shields.io/github/languages/top/meli/mailpot?color=white&logo=rust&logoColor=black
[License]: https://img.shields.io/github/license/meli/mailpot?color=white
[docs.rs]: https://img.shields.io/docsrs/mailpot?color=white
[rustdoc]: https://meli.github.io/mailpot/docs/mailpot/
[Coverage]: https://img.shields.io/endpoint?color=white&url=https://meli.github.io/mailpot/coverage/coverage.json
[grcov-rport]: https://meli.github.io/mailpot/coverage/

- Official hosted instance of `mailpot-web` crate: <https://lists.meli.delivery/>
- Rendered rustdoc: <https://meli.github.io/mailpot/docs/mailpot/>
- CLI manpage: [`mpot.1`](./docs/mpot.1) [Rendered](https://git.meli.delivery/meli/mailpot/src/branch/main/docs/mpot.1)

| ℹ️  Interested in contributing? Consult [`CONTRIBUTING.md`](./CONTRIBUTING.md). |
| ---                                                                            |

## crates:

- `core` the library
- `cli` a command line tool to manage lists
- `web` an `axum` based web server capable of serving archives and authenticating list owners and members
- `archive-http` static web archive generation or with a dynamic http server
- `rest-http` a REST http server to manage lists

## Features

- easy setup
- extensible through Rust API as a [library](./core)
- basic management through [CLI tool](./cli/)
- optional lightweight web archiver ([static](./archive-http/) and [dynamic](./web/))
- useful for both **newsletters**, **communities** and for static **article comments**

## Roadmap

- extensible through HTTP REST API as an HTTP server, with webhooks

## Initial setup

Create a configuration file and a database:

```shell
$ mkdir -p /home/user/.config/mailpot
$ export MPOT_CONFIG=/home/user/.config/mailpot/config.toml
$ cargo run --bin mpot -- sample-config > "$MPOT_CONFIG"
$ # edit config and set database path e.g. "/home/user/.local/share/mailpot/mpot.db"
$ cargo run --bin mpot -- -c "$MPOT_CONFIG" list-lists
No lists found.
```

This creates the database file in the configuration file as if you executed the following:

```shell
$ sqlite3 /home/user/.local/share/mailpot/mpot.db < ./core/src/schema.sql
```

## Examples

```text
% mpot help
GNU Affero version 3 or later <https://www.gnu.org/licenses/>

Tool for mailpot mailing list management.

Usage: mpot [OPTIONS] <COMMAND>

Commands:
  sample-config
          Prints a sample config file to STDOUT
  dump-database
          Dumps database data to STDOUT
  list-lists
          Lists all registered mailing lists
  list
          Mailing list management
  create-list
          Create new list
  post
          Post message from STDIN to list
  flush-queue
          Flush outgoing e-mail queue
  error-queue
          Mail that has not been handled properly end up in the error queue
  queue
          Mail that has not been handled properly end up in the error queue
  import-maildir
          Import a maildir folder into an existing list
  update-postfix-config
          Update postfix maps and master.cf (probably needs root permissions)
  print-postfix-config
          Print postfix maps and master.cf entry to STDOUT
  accounts
          All Accounts
  account-info
          Account info
  add-account
          Add account
  remove-account
          Remove account
  update-account
          Update account info
  repair
          Show and fix possible data mistakes or inconsistencies
  help
          Print this message or the help of the given subcommand(s)

Options:
  -d, --debug
          Print logs

  -c, --config <CONFIG>
          Configuration file to use

  -q, --quiet
          Silence all output

  -v, --verbose...
          Verbose mode (-v, -vv, -vvv, etc)

  -t, --ts <TS>
          Debug log timestamp (sec, ms, ns, none)

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

### Receiving mail

```shell
$ cat list-request.eml | cargo run --bin mpot -- -vvvvvv post --dry-run
```

<details><summary>output</summary>

```shell
TRACE - Received envelope to post: Envelope {
    Subject: "unsubscribe",
    Date: "Tue, 04 Aug 2020 14:10:13 +0300",
    From: [
        Address::Mailbox {
            display_name: "Mxxxx Pxxxxxxxxxxxx",
            address_spec: "exxxxx@localhost",
        },
    ],
    To: [
        Address::Mailbox {
            display_name: "",
            address_spec: "test-announce+request@localhost",
        },
    ],
    Message-ID: "<ejduu.fddf8sgen4j7@localhost>",
    In-Reply-To: None,
    References: None,
    Hash: 12581897380059220314,
}
TRACE - unsubscribe action for addresses [Address::Mailbox { display_name: "Mxxxx Pxxxxxxxxxxxx", address_spec: "exxxxx@localhost" }] in list [#2 test-announce] test announcements <test-announce@localhost>
TRACE - Is post related to list [#1 test] Test list <test@localhost>? false
```
</details>

```shell
$ cat list-post.eml | cargo run --bin mpot -- -vvvvvv post --dry-run
```

<details><summary>output</summary>

```shell
TRACE - Received envelope to post: Envelope {
    Subject: "[test-announce] new test releases",
    Date: "Tue, 04 Aug 2020 14:10:13 +0300",
    From: [
        Address::Mailbox {
            display_name: "Mxxxx Pxxxxxxxxxxxx",
            address_spec: "exxxxx@localhost",
        },
    ],
    To: [
        Address::Mailbox {
            display_name: "",
            address_spec: "test-announce@localhost",
        },
    ],
    Message-ID: "<ejduu.sddf8sgen4j7@localhost>",
    In-Reply-To: None,
    References: None,
    Hash: 10220641455578979007,
}
TRACE - Is post related to list [#1 test] Test list <test@localhost>? false
TRACE - Is post related to list [#2 test-announce] test announcements <test-announce@localhost>? true
TRACE - Examining list "test announcements" <test-announce@localhost>
TRACE - List subscriptions [
    ListSubscription {
        list: 2,
        address: "exxxxx@localhost",
        name: None,
        digest: false,
        hide_address: false,
        receive_duplicates: false,
        receive_own_posts: true,
        receive_confirmation: true,
        enabled: true,
    },
]
TRACE - Running FixCRLF filter
TRACE - Running PostRightsCheck filter
TRACE - Running AddListHeaders filter
TRACE - Running FinalizeRecipients filter
TRACE - examining subscription ListSubscription { list: 2, address: "exxxxx@localhost", name: None, digest: false, hide_address: false, receive_duplicates: false, receive_own_posts: true, receive_confirmation: true, enabled: true }
TRACE - subscription is submitter
TRACE - subscription gets copy
TRACE - result Ok(
    Post {
        list: MailingList {
            pk: 2,
            name: "test announcements",
            id: "test-announce",
            address: "test-announce@localhost",
            description: None,
            archive_url: None,
        },
        from: Address::Mailbox {
            display_name: "Mxxxx Pxxxxxxxxxxxx",
            address_spec: "exxxxx@localhost",
        },
        subscriptions: 1,
        bytes: 851,
        policy: None,
        to: [
            Address::Mailbox {
                display_name: "",
                address_spec: "test-announce@localhost",
            },
        ],
        action: Accept {
            recipients: [
                Address::Mailbox {
                    display_name: "",
                    address_spec: "exxxxx@localhost",
                },
            ],
            digests: [],
        },
    },
)
```
</details>

## Using `mailpot` as a library

```rust
use mailpot::{models::*, *};
use tempfile::TempDir;

let tmp_dir = TempDir::new().unwrap();
let db_path = tmp_dir.path().join("mpot.db");
let config = Configuration {
    send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
    db_path: db_path.clone(),
    data_path: tmp_dir.path().to_path_buf(),
    administrators: vec!["myaddress@example.com".to_string()],
};
let db = Connection::open_or_create_db(config)?.trusted();

// Create a new mailing list
let list_pk = db.create_list(MailingList {
    pk: 0,
    name: "foobar chat".into(),
    id: "foo-chat".into(),
    address: "foo-chat@example.com".into(),
    description: None,
    topics: vec![],
    archive_url: None,
})?.pk;

db.set_list_post_policy(
    PostPolicy {
        pk: 0,
        list: list_pk,
        announce_only: false,
        subscription_only: true,
        approval_needed: false,
        open: false,
        custom: false,
    },
)?;

// Drop privileges; we can only process new e-mail and modify subscriptions from now on.
let mut db = db.untrusted();

assert_eq!(db.list_subscriptions(list_pk)?.len(), 0);
assert_eq!(db.list_posts(list_pk, None)?.len(), 0);

// Process a subscription request e-mail
let subscribe_bytes = b"From: Name <user@example.com>
To: <foo-chat+subscribe@example.com>
Subject: subscribe
Date: Thu, 29 Oct 2020 13:58:16 +0000
Message-ID: <1@example.com>

";
let envelope = melib::Envelope::from_bytes(subscribe_bytes, None)?;
db.post(&envelope, subscribe_bytes, /* dry_run */ false)?;

assert_eq!(db.list_subscriptions(list_pk)?.len(), 1);
assert_eq!(db.list_posts(list_pk, None)?.len(), 0);

// Process a post
let post_bytes = b"From: Name <user@example.com>
To: <foo-chat@example.com>
Subject: my first post
Date: Thu, 29 Oct 2020 14:01:09 +0000
Message-ID: <2@example.com>

Hello
";
let envelope =
    melib::Envelope::from_bytes(post_bytes, None).expect("Could not parse message");
db.post(&envelope, post_bytes, /* dry_run */ false)?;

assert_eq!(db.list_subscriptions(list_pk)?.len(), 1);
assert_eq!(db.list_posts(list_pk, None)?.len(), 1);
# Ok::<(), Error>(())
```
