# mailpot-core

Initialize `sqlite3` database

```shell
sqlite3 mpot.db < ./src/schema.sql
```

## Tests

`test_smtp_mailcrab` requires a running mailcrab instance.
You must set the environment variable `MAILCRAB_IP` to run this.
Example:

```shell
MAILCRAB_IP="127.0.0.1" cargo test mailcrab
```
