# Mailpot - WIP mailing list manager

Crates:

- `core`
- `cli` a command line tool to manage lists
- `rest-http` a REST http server to manage lists

## Project goals

- easy setup
- extensible through Rust API as a library
- extensible through HTTP REST API as an HTTP server, with webhooks
- basic management through CLI
- replaceable lightweight web archiver
- custom storage?
- useful for both newsletters, discussions

## Initial setup

Check where `mpot` expects your database file to be:

```shell
$ cargo run --bin mpot -- db-location
Configuration file /home/user/.config/mailpot/config.toml doesn't exist
```

Uuugh, oops.

```shell
$ mkdir -p /home/user/.config/mailpot
$ echo 'send_mail = { "type" = "ShellCommand", "value" = "/usr/bin/false" }' > /home/user/.config/mailpot/config.toml
$ cargo run --bin mpot -- db-location
/home/user/.local/share/mailpot/mpot.db
```

Now you can initialize the database file:

```shell
$ mkdir -p /home/user/.local/share/mailpot/
$ sqlite3 /home/user/.local/share/mailpot/mpot.db < ./core/src/schema.sql
```

## Examples

```text
% mpot help
mailpot 0.1.0
mini mailing list manager

USAGE:
    mpot [FLAGS] [OPTIONS] <SUBCOMMAND>

FLAGS:
    -d, --debug      Activate debug mode
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --config <config>    Set config file

SUBCOMMANDS:
    create-list    Create new list
    db-location    Prints database filesystem location
    help           Prints this message or the help of the given subcommand(s)
    list           Mailing list management
    list-lists     Lists all registered mailing lists
    post           Post message from STDIN to list
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
TRACE - List members [
    ListMembership {
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
TRACE - examining member ListMembership { list: 2, address: "exxxxx@localhost", name: None, digest: false, hide_address: false, receive_duplicates: false, receive_own_posts: true, receive_confirmation: true, enabled: true }
TRACE - member is submitter
TRACE - Member gets copy
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
        members: 1,
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
