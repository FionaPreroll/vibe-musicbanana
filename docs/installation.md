# Installation and operation

[Back to the README](../README.md)

- [Running with Docker](#running-with-docker)
- [Connecting Navidrome](#connecting-navidrome)
- [Optional services](#optional-services): YourSpotify, album covers
- [Moving existing data in](#moving-existing-data-in)
- [Updates, backups, logs](#updates-backups-logs)
- [Without Docker](#without-docker)
- [Settings](#settings)
- [Logs](#logs)
- [Status page](#status-page)

## Running with Docker

`Dockerfile` builds one image with the server and the frontend. CI publishes it for every change on main that passed all checks, as `ghcr.io/fionapreroll/vibe-musicbanana:latest` and `:sha-<commit>`, for x86-64 (linux/amd64). `deploy/compose.yaml` runs it together with its own PostgreSQL. On the machine that runs Navidrome:

```sh
git clone https://github.com/FionaPreroll/vibe-musicbanana.git musicbanana
cd musicbanana/deploy
cp .env.example .env    # set MUSICBANANA_DB_PASSWORD, e.g. to the output of `openssl rand -hex 24`
docker login ghcr.io    # once: your GitHub user name, and a token with read:packages as password
docker compose pull
docker compose up -d
```

The image is private like the repository, hence the login; once the package is public (GitHub, package settings, "Change visibility"), pulling needs none, and the code stays private. An image that could not be pulled is built from the checkout instead, which takes a while (a Rust release build); `docker compose up -d --build` always builds.

Afterwards the web interface is at `http://<server>:3000`; `MUSICBANANA_PORT` in `.env` changes the port. Commands of the binary run inside the container, for example a new account (`-it` so that it can ask for the password):

```sh
docker compose exec -it musicbanana musicbanana account create <username> --email <address>
```

All other commands in these docs work the same way: prefix them with `docker compose exec musicbanana`. After logging in, the settings page makes the token for Navidrome.

## Connecting Navidrome

Navidrome needs the address under which its container reaches musicbanana, as `ND_LISTENBRAINZ_BASEURL` in its environment:

- through the published port, with the server's address in your network: `http://192.168.1.10:3000/api/listenbrainz/1/` (not `localhost`, which inside the Navidrome container is the container itself);
- or, when both containers share a Docker network (for example with the Navidrome service in the same compose file), by service name: `http://musicbanana:3000/api/listenbrainz/1/`.

Then restart Navidrome and link the token as described in [Scrobbling](scrobbling.md#navidrome-and-apps-that-play-from-it-supersonic-ultrasonic-).

## Optional services

**Spotify plays from YourSpotify** (see [Spotify via YourSpotify](spotify.md)): the simplest way is a [connection](spotify.md#connections-the-server-keeps-up-to-date) on the settings page, which the `musicbanana` container keeps up to date by itself; set `YOURSPOTIFY_ALLOWED_URLS` in `.env` so the settings page takes the address. The older way still works: with these lines in `.env`, `docker compose up -d` also starts the service `yourspotify`, which imports the history once and then the new plays every 15 minutes (`YOURSPOTIFY_EVERY` changes that). The address is the one under which the container reaches YourSpotify's API, as for Navidrome not `localhost`. `docker compose logs yourspotify` shows what it imported.

```sh
COMPOSE_PROFILES=yourspotify
YOURSPOTIFY_URL=http://192.168.1.10:8080
YOURSPOTIFY_TOKEN=<public token>
YOURSPOTIFY_USER=<username>
```

**Album covers** (see [Album covers](covers.md)): `COVER_ART=on` in `.env`, `COVER_ART_LOOKUP=on` for the searches by name and `COVER_ART_CACHE_MAX` for another limit than 1G. The covers are kept in the volume `covers` (`/var/cache/musicbanana` in the container), so they survive updates; `docker compose down -v` or `docker volume rm musicbanana_covers` empties it, and it needs no backup.

## Moving existing data in

For example the database the [PHP import](import-php.md) went into:

```sh
pg_dump -Fc -d postgres://postgres@127.0.0.1:5432/musicbanana -f musicbanana.dump   # where the data is now
docker compose stop musicbanana
docker compose exec -T db pg_restore -U musicbanana -d musicbanana --clean --if-exists --no-owner < musicbanana.dump
docker compose start musicbanana
```

## Updates, backups, logs

**Updates:** `docker compose pull && docker compose up -d`, or `git pull && docker compose up -d --build` to build it yourself. Migrations run when the server starts.

**Backups:** `docker compose exec -T db pg_dump -U musicbanana -Fc musicbanana > musicbanana-$(date +%F).dump`, restored as [above](#moving-existing-data-in).

**Logs:** `docker compose logs -f musicbanana`, see [Logs](#logs).

## Without Docker

```sh
cd frontend && pnpm install --frozen-lockfile && pnpm build
cd backend && cargo build --release
STATIC_DIR=../frontend/build DATABASE_URL=postgres://… ./target/release/musicbanana
```

## Settings

The server reads its settings from the environment; with Docker they go into `deploy/.env`.

| Variable | Default | What |
|---|---|---|
| `DATABASE_URL` | (required) | PostgreSQL 16+, e.g. `postgres://musicbanana:…@db:5432/musicbanana`. With Docker, compose builds it from `MUSICBANANA_DB_PASSWORD`. |
| `LISTEN_ADDR` | `127.0.0.1:3000` (`0.0.0.0:3000` in the image) | Address and port of the web interface and API. With Docker, `MUSICBANANA_PORT` picks the port on the host. |
| `STATIC_DIR` | `../frontend/build` (`/usr/share/musicbanana` in the image) | The built frontend. |
| `ACCESS_LOG` | `on` | One log line per request, see [Logs](#logs). |
| `RUST_LOG` | `musicbanana=info,tower_http=info` | What is logged, see [Logs](#logs). |
| `YOURSPOTIFY_ALLOWED_URLS` | (none) | YourSpotify addresses the settings page takes, separated by commas, see [Spotify](spotify.md#connections-the-server-keeps-up-to-date). |
| `COVER_ART` | `off` | `on` shows album covers, see [Album covers](covers.md). |
| `COVER_ART_LOOKUP` | `off` | `on` also searches MusicBrainz by name for albums without an ID; needs `COVER_ART=on`. |
| `COVER_ART_CACHE` | `cache/covers` (`/var/cache/musicbanana/covers` in the image) | Where covers are kept. |
| `COVER_ART_CACHE_MAX` | `1G` | The most the cover cache may hold, at least `10M`; the covers seen longest ago go first. |

## Logs

The server logs to standard output (`docker compose logs -f musicbanana`). Besides start-up and errors:

- `musicbanana::access`: one line per request, with the client (the first `X-Forwarded-For` address behind a reverse proxy), method, path with query, status and milliseconds, e.g. `192.168.1.20 POST /api/me/profiles 201 12 ms`. Health checks and the frontend's files are left out. `ACCESS_LOG=off` switches it off.
- `musicbanana::account`: logins (failed ones as warnings, with the name tried), logouts and every change in the settings: profiles, scrobble tokens, YourSpotify connections (never their token), user name and password.
- `musicbanana::merge`: merges, undos and hidden suggestions from the page Merges, with the admin's account id.
- `musicbanana::connections`: each round of a YourSpotify connection, when it starts and what it brought, or why it failed; the import's progress comes from `musicbanana::yourspotify`.

`RUST_LOG` picks what is logged, by target and level: `RUST_LOG=musicbanana=info,musicbanana::account=warn` keeps only failed logins of the settings, `RUST_LOG=musicbanana=debug` logs more, and `musicbanana::access=off` is the same as `ACCESS_LOG=off`.

## Status page

Admins have a Status link at the top that leads to `/status`: the build (version and commit), the server's CPU and memory, the machine's load and memory, the database (size, migration, connections, cache hits, rows and size per table, listens of the last 24 hours by source), the YourSpotify connections and how long each API route took since the server started (mean, median, 95th percentile, maximum and 5xx errors). It refreshes every 10 seconds. `musicbanana account admin <username>` makes an account an admin, `--off` takes that back; nobody else sees the page, nor the page [Merges](catalog.md#in-the-browser).

The commit comes from the image build (`--build-arg GIT_COMMIT=…`, which CI sets); a local build shows none.
