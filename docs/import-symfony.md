# Importing the old musicbanana-symfony database

[Back to the README](../README.md)

The Symfony version of musicbanana (2021-2022) kept its data in PostgreSQL 12. Its listens can be added to a musicbanana that is already in use, next to the [PHP import](import-php.md), Navidrome and YourSpotify. Accounts and passwords are not imported. The old version showed all profiles of an account together, one per player or import, which is what sources are here (`?source=`, see [Pages](pages.md)). So all old profiles of an account go into the default profile of the account of the same name (or the one named with `--user`), each as a source of its own. The importer reads the tables `users`, `profile` and `scrobble`.

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
fiona/_ -> fiona/default as import:symfony-2022
  listens:        4
  same second:    1 (left out)
  same track:     1 (moments apart, left out)
  new:            2
  1 or 2 h off:   0 of the new ones (time zone? see docs/import-symfony.md)
fiona/Arbeit -> fiona/default as import:symfony-2022/Arbeit
  listens:        2
  same second:    1 (left out)
  same track:     1 (moments apart, left out)
  PHP import:     2 of these take the source import:symfony-2022/Arbeit
  ...
```

- An old profile goes to the default profile of the account of the same name, or of the one `--user OLD_USER=USER` names when the account has been renamed since. Its listens get the source `import:symfony-2022/<old profile>`, which reads "Old musicbanana (2022) · <old profile>" on the pages. `_`, the old version's name for each account's first profile, is the old musicbanana itself and gets plain `import:symfony-2022`. Old accounts with no account to go to are skipped: create the account first (`musicbanana account create`), or send a profile somewhere else with `--profile OLD_USER/OLD_PROFILE=USER/SLUG`. `--skip OLD_USER/OLD_PROFILE` leaves a profile out. `--user`, `--profile` and `--skip` can each be given several times.
- Plays the account already has are left out, in any of its profiles. That covers a listen at the same second as one already there (the years the PHP import already brought, for example), and a listen of the same artist and title a few moments from one already there, closer than half the track's length or four minutes (from Navidrome or YourSpotify, for example). The same applies between the old profiles, so a play that is in two of them is imported once, from the first. Listens the owner moved to the trash stay out as well. A second run therefore adds nothing.
- When a play left out that way is a listen of the PHP import and the old profile has a name, that listen takes the old profile's source, since the old version knew where it came from (a last.fm export, say). Listens from players and YourSpotify keep their source, and so do PHP listens that match `_`.
- **1 or 2 h off** counts new listens that have a listen exactly one or two hours away. If that number is large, one of the two imports stored local time instead of UTC, and the same plays would show up twice. In that case use `--since`/`--until` (UTC days, e.g. `--since 2016-05-31`) to import only the years the other side does not have.
- Double-encoded umlauts ("Die Ã„rzte") are repaired as in the PHP import. Lengths above 20,000 count as milliseconds, 0 as unknown, and all others as seconds.

Without `--dry-run` it writes and also prints the number of new listens per profile. Afterwards `db2022` can be stopped and removed with `docker compose -f compose.yaml -f compose.db2022.yaml rm -s db2022`.

Outside Docker the importer runs like the others: `cargo run --release -- import-symfony --from postgres://…` in `backend/`.

The tests (`tests/import_symfony.rs`) load a small made-up old database (`tests/fixtures/symfony2022.sql`) into a database of their own on the server of `DATABASE_URL`.
