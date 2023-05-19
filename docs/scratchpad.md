# Ideas, plans, thoughts on `mailpot`.

It'd be better if this stuff wasn't on an issue tracker like gitea's or 
github's but committed in the repository.

Discussion about these notes can take place in the mailing list,
 [`<mailpot-general@meli.delivery>`](https://lists.meli.delivery/list/mailpot-general/).

In no particular order:

**Table of contents**:

* [Possible Postfix integrations](#possible-postfix-integrations)
* [Setup docker container network with postfix for testing](#setup-docker-container-network-with-postfix-for-testing)
* [Add NNTP gateways](#add-nntp-gateways)
* [Add MIME type filter for list owners](#add-mime-type-filter-for-list-owners)
* [Add `convert_html_to_plaintext` filter](#add-convert_html_to_plaintext-filter)
* [Use mdoc instead of roff for manpages](#use-mdoc-instead-of-roff-for-manpages)
* [Add shell completions with `clap`](#add-shell-completions-with-clap)
* [Make complex database logic and/or complex migrations with user defined functions](#make-complex-database-logic-andor-complex-migrations-with-user-defined-functions)
* [Implement dtolnay's mailing set concept](#implement-dtolnays-mailing-set-concept)

## Possible Postfix integrations

- local delivery with `postdrop(1)` instead of SMTP
- log with `postlog(1)`
- sqlite maps <https://www.postfix.org/SQLITE_README.html>

## Setup docker container network with postfix for testing

Beyond integration tests, we need a real-world testcase: a bunch of user postfixes talking to a mailing list postfix.
This can be done with a docker setup.
A simple debian slim image can be used for this.
Reference for postfix on docker: <https://www.frakkingsweet.com/postfix-in-a-container/>.
It'd be great if we could use a Rust based solution as well, with something like <https://github.com/fussybeaver/bollard>.

## Add NNTP gateways

TODO

## Add MIME type filter for list owners

TODO

## Add `convert_html_to_plaintext` filter

TODO

## Use mdoc instead of roff for manpages

[`mdoc` reference](https://man.openbsd.org/mdoc.7)

Progress:

- Got ownership of `mdoc` on crates.io.
- Forked `roff` crate to use as a basis: <https://github.com/epilys/mdoc>

## Add shell completions with `clap`

Probably with <https://docs.rs/clap_complete/latest/clap_complete/>

## Make complex database logic and/or complex migrations with user defined functions

Useful projects:

- <https://github.com/facebookincubator/CG-SQL/tree/main>
- <https://github.com/epilys/vfsstat.rs>

## Implement dtolnay's mailing set concept

See <https://github.com/dtolnay/mailingset/tree/master>

> A mailing list server that treates mailing lists as sets and allows mail to
> be sent to the result of set-algebraic expressions on those sets. The union,
> intersection, and difference operators are supported. Sending mail to a set
> operation involves specifying a set expression in the local part of the
> recipient email address.
