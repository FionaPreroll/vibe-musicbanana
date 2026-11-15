# Importing the old musicbanana-symfony database

[Back to the README](../README.md)

The Symfony version of musicbanana (2021-2022) kept its data in PostgreSQL 12. Its listens can be added to a musicbanana that is already in use, next to the [PHP import](import-php.md), Navidrome and YourSpotify. Accounts and passwords are not imported. The old profiles go to the accounts of the same names.

## The old database next to the new one

[`deploy/compose.db2022.yaml`](../deploy/compose.db2022.yaml) adds the old database to the stack as the service `db2022`. It runs `postgres:12`, publishes no ports, and is reachable only from the stack's own containers, as `db2022:5432`. Next to `compose.yaml`:

```sh
mkdir -p db2022/backup db2022/secrets
cp <old stack>/secrets/db_user <old stack>/secrets/db_password <old stack>/secrets/db_dbname db2022/secrets/
cp <the backup> db2022/backup/
cp <this repository>/deploy/db2022/restore.sh db2022/
docker compose -f compose.yaml -f compose.db2022.yaml up -d db2022
```

The secret files are the old stack's own, so the database user that owns the old tables exists again. On its first start the container restores everything in `db2022/backup`. Plain SQL (`.sql`, `.sql.gz`) goes through psql, and pg_dump's custom, directory and tar formats go through pg_restore ([`restore.sh`](../deploy/db2022/restore.sh)). If the backup is a copy of the old data directory and not a dump, mount it at `/var/lib/postgresql/data` instead of the `db2022` volume and leave `restore.sh` out. On its first start PostgreSQL then recovers it if the old server was not shut down cleanly.

The service and its `secrets:` and `volumes:` entries can also be copied into `compose.yaml`. If the stack defines its own networks, `db2022` needs to be on the same one as `musicbanana`.

## Import

Try it with `--dry-run` first. The importer reads the password from `SYMFONY_DATABASE_URL`, so it stays out of the shell history:

```sh
docker compose -f compose.yaml -f compose.db2022.yaml run --rm \
  -e SYMFONY_DATABASE_URL="postgres://$(cat db2022/secrets/db_user):$(cat db2022/secrets/db_password)@db2022:5432/$(cat db2022/secrets/db_dbname)" \
  musicbanana import-symfony --dry-run
```

For each old profile it prints where its listens go and how they line up with the listens already there:

```text
fiona/Default -> fiona/default
  listens:        4
  same second:    1 (left out)
  < 4 min apart:  1 (left out if the same track)
  1 or 2 h off:   0 (time zone? see docs/import-symfony.md)
fiona/Arbeit -> fiona/arbeit (new profile)
  ...
```

- An old profile goes to the profile of the same name or address on the account of the same name. If the old account had only one profile, it goes to the default profile. Otherwise the import creates a new profile with the old name, which is private if the old profile was not public. Old accounts with no account of the same name are skipped. Create the account first (`musicbanana account create`), or name the target with `--profile OLD_USER/OLD_PROFILE=USER/SLUG`, which can be given several times.
- The import goes through the same path as scrobbles. A listen the profile already has at the same second is left out, which covers the years the PHP import already brought. So is a listen of the same track a few moments from one that is already there, from Navidrome or YourSpotify for example, and any listen the owner moved to the trash. A second run therefore adds nothing.
- **1 or 2 h off** counts listens that have no match nearby but have one exactly one or two hours away. If that number is large, one of the two imports stored local time instead of UTC, and the same plays would show up twice. In that case use `--since`/`--until` (UTC days, e.g. `--since 2016-05-31`) to import only the years the other side does not have.
- Double-encoded umlauts ("Die Ã„rzte") are repaired as in the PHP import. Lengths above 20,000 count as milliseconds, all others as seconds.

Without `--dry-run` it writes and also prints the number of new listens per profile. They show up with the source "Old musicbanana (2022)" (`import:symfony-2022`). Afterwards `db2022` can be stopped and removed with `docker compose -f compose.yaml -f compose.db2022.yaml rm -s db2022`.

Outside Docker the importer runs like the others: `cargo run --release -- import-symfony --from postgres://…` in `backend/`.

The tests (`tests/import_symfony.rs`) load a small made-up old database (`tests/fixtures/symfony2022.sql`) into a database of their own on the server of `DATABASE_URL`.
