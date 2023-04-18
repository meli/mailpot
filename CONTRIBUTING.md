# Contributing to mailpot

Contributions are welcome and encouraged.
They can be anything from spelling corrections, art, documentation, or source code fixes & additions.
If a source code contribution is correct, functional and follows the code style and feature goals of the rest of the project, it will be merged.

**Table of contents**:

- [Important links](#important-links)
- [Developing environment](#developing-environment)
- [Testing](#testing)
- [How to submit changes](#how-to-submit-changes)
- [Choosing what to work on](#choosing-what-to-work-on)
- [How to request an enhancement, new features](#how-to-request-an-enhancement-new-features)
- [Style Guide / Coding conventions](#style-guide--coding-conventions)
- [Specific questions and answers](#specific-questions-and-answers)
  - [How do I include new images / icons?](#how-do-i-include-new-images--icons)

## Important links

- Main repository: <https://git.meli.delivery/meli/mailpot>
- Bug/Issue tracker: <https://git.meli.delivery/meli/mailpot/issues>
- Mailing list: <https://lists.meli.delivery/list/mailpot-general/>

To privately contact the repository's owner, check their commit history for their e-mail address.

<sup><sub><a href="#contributing-to-mailpot">back to top</a></sub></sup>

## Developing environment

You will need a UNIX-like operating system that is supported by Rust.
You can install rust and cargo with the [`rustup`](https://rustup.rs) tool.

<sup><sub><a href="#contributing-to-mailpot">back to top</a></sub></sup>

## Testing

All tests can be executed with `cargo`.
Run

```shell
cargo test --all --no-fail-fast --all-features
```

to run all tests.

<sup><sub><a href="#contributing-to-mailpot">back to top</a></sub></sup>

## How to submit changes

Use gitea's PR functionality.
Alternatively, submit patches to the mailing list.

<sup><sub><a href="#contributing-to-mailpot">back to top</a></sub></sup>

## Choosing what to work on

You can find some tasks in the bug tracker.
Additionally, tasks are annotated inside the source code with the keywords `FIXME`, `TODO` and others.
For a list of all tags search for `[tag:`.
For a list of all references search for `[ref:`.
To find tag references you can use a text search tool of your choice such as `grep`, `ripgrep` or others.
The CLI tool `tagref` can also be used:

```shell
/path/to/mailpot $ tagref list-refs
[ref:FIXME] @ ./src/module.rs:106
[ref:FIXME] @ ./src/module.rs:867
[ref:FIXME] @ ./src/module.rs:30
[ref:TODO] @ ./src/where.rs:411
...
```

You can of course filter or sort them by tag:

```shell
/path/to/mailpot $ tagref list-refs | grep TODO
...
/path/to/mailpot $ tagref list-refs | sort -u
...
```

<sup><sub><a href="#contributing-to-mailpot">back to top</a></sub></sup>

## How to request an enhancement, new features

Simply open a new issue on the bug tracker or post on the mailing list.

<sup><sub><a href="#contributing-to-mailpot">back to top</a></sub></sup>

## Style Guide / Coding conventions

All Rust code must be formatted by `rustfmt`, and pass clippy lints.

```shell
cargo check --all-features --all --tests --examples --benches --bins
cargo +nightly fmt --all || cargo fmt --all
cargo clippy --no-deps --all-features --all --tests --examples --benches --bins
djhtml -i web/src/templates/* || printf "djhtml binary not found in PATH.\n"
```

<sup><sub><a href="#contributing-to-mailpot">back to top</a></sub></sup>
