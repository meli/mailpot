# mailpot web server

```shell
cargo run --bin mpot-web -- /path/to/conf.toml
```

By default on release builds templates are compressed with `zstd` and bundled in the binary.

You can disable this behavior by disabling the `zstd` feature: `cargo build --release --no-default-features`

## Configuration

By default, the server listens on `0.0.0.0:3000`.
The following environment variables can be defined to configure various settings:

- `HOSTNAME`, default `0.0.0.0`.
- `PORT`, default `3000`.
- `PUBLIC_URL`, default `lists.mailpot.rs`.
- `SITE_TITLE`, default `mailing list archive`.
- `SITE_SUBTITLE`, default empty.
- `ROOT_URL_PREFIX`, default empty.
- `SSH_NAMESPACE`, default `lists.mailpot.rs`.
