# mailpot-core

Initialize `sqlite3` database:

Either

```shell
sqlite3 mpot.db < ./src/schema.sql
```

or


```shell
# cargo install diesel_cli --no-default-features --features sqlite
diesel migration run
```
