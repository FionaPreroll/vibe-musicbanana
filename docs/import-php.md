# Importing the old musicbanana-php database

[Back to the README](../README.md)

The importer reads the `mb_*` tables of the 2016 musicbanana-php database straight from MySQL/MariaDB and writes into an empty musicbanana database (it refuses to run twice). It repairs double-encoded names, follows old merges and wraps the old MD5 password hashes in argon2id, so the old accounts log in with their old passwords; details in [design.md](design.md) (German).

```sh
cd backend
DATABASE_URL=postgres://postgres@127.0.0.1:5432/musicbanana \
  cargo run --release -- import-php --from mysql://root@127.0.0.1:3306/musicbanana
```

It prints a summary (accounts, artists, listens, skipped rows, repaired names). Afterwards the history is at `/u/<old username>`, with the source `import:php-2016`. To run it on a server with Docker, import into a local database first and [move it in](installation.md#moving-existing-data-in).

The import test (`tests/import_php.rs`) runs against a MySQL server named by `LEGACY_MYSQL_URL` and is skipped without it.
