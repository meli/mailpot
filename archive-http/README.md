# mailpot REST http server

```shell
cargo run --bin mpot-archives
```

## generate static files

```shell
# mpot-gen CONF_FILE OUTPUT_DIR OPTIONAL_ROOT_URL_PREFIX
cargo run --bin mpot-gen -- ../conf.toml ./out/ "/mailpot"
```
