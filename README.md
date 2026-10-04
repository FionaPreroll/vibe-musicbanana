# musicbanana

A free place for your listening history: scrobbling and listening charts, running in one form or another since 2007.

Design notes and decisions: [docs/design.md](docs/design.md) (German).

## Layout

| Path | What |
|---|---|
| `backend/` | Rust: Axum HTTP server, sqlx + PostgreSQL. Serves `/api/*` and the built frontend. |
| `backend/migrations/` | SQL migrations, applied automatically on startup. |
| `backend/.sqlx/` | Offline query cache so the crate builds without a database (`cargo sqlx prepare`). |
| `frontend/` | SvelteKit + TypeScript + Tailwind, built as a static SPA (`adapter-static`). |
| `Dockerfile`, `deploy/` | The image with server and frontend, and a compose file to run it with PostgreSQL ([Running with Docker](#running-with-docker)). |

## Development

Requirements: Rust (stable), Node 22+, pnpm (e.g. `npm install -g pnpm`; it switches to the version pinned in `frontend/package.json` by itself), Docker (or any PostgreSQL 16+).

```sh
docker compose up -d db                       # PostgreSQL on localhost:5432
cp backend/.env.example backend/.env

cd backend && cargo run                       # applies migrations, API on http://127.0.0.1:3000
cd frontend && pnpm install && pnpm dev       # UI on http://localhost:5173, proxies /api
```

### Changing SQL

sqlx checks every `query!` at compile time. Builds use the committed query cache in `backend/.sqlx/` (`SQLX_OFFLINE=true` in `.cargo/config.toml`), so they work without a database. After adding or changing a query or a migration, refresh the cache against a migrated database and commit the updated `.sqlx/` files:

```sh
cargo install sqlx-cli --version '~0.9' --no-default-features --features postgres,rustls   # once
cd backend && cargo sqlx migrate run && cargo sqlx prepare
```

Until then the build fails with "no cached data for this query". To check queries live while editing, run with `SQLX_OFFLINE=false` against a migrated database.

Checks (the same ones CI runs):

```sh
cd backend && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd frontend && pnpm lint && pnpm check && pnpm build
```

## Pages and API

| Page | Data from |
|---|---|
| `/` lists the public profiles | `GET /api/profiles` |
| `/u/<username>` (default profile) or `/u/<username>/<slug>`: listens per year, top artists, albums and tracks, the top artists of each year, recent listens, what is playing now; `?days=30` (also 7, 90, 365), `?year=2012` or `?from=2009-06-01&to=2009-08-31` narrows the top lists and listens to that period | `GET /api/profiles/<username>/<slug>?tz=`, `…/top/{artists,releases,recordings}?year=&from=&to=&tz=&limit=`, `…/top/artists/years?tz=&limit=`, `…/listens?before=&limit=`, `…/now-playing` |
| `/u/<username>/artist/<id>`, `…/album/<id>` and `…/track/<id>` (after the slug for other profiles): listens per month, first and last listen, phases of heavy listening, the albums and tracks heard | `GET /api/profiles/<username>/<slug>/{artists,releases,recordings}/<id>?tz=` |

Only public profiles are served until there is a login. Years and days start at midnight in `tz` (an IANA name such as `Europe/Berlin`, UTC by default); the frontend sends the browser's time zone. `from` and `to` are the first and last day of a period, both included, and either can be left out; `year=2012` is short for the whole year. `listens` pages backwards: pass a page's `next` as `before`. `now-playing` is `null` when nothing plays; the open page asks again every 30 seconds and adds new listens on top. The months of an artist, album or track run from the profile's first listen to its last, leaving out a year or more without any listens (shown as a break); an entry that was merged into another one answers with that one.

## Scrobbling

musicbanana speaks the part of the [ListenBrainz API](https://listenbrainz.readthedocs.io/en/latest/users/api/core.html) that players use to scrobble, under `/api/listenbrainz/1/`: `POST submit-listens` (`single`, `import` and `playing_now`) and `GET validate-token`. Limits, checks and error responses follow listenbrainz-server. A listen keeps the strings and extra data (`additional_info`) exactly as sent and is matched to the catalog through the alias tables; unknown artists, albums and tracks are created. A second listen at the same second is skipped, so clients can safely resend. So is a listen of the same track that follows another one sooner than a player counts a play (half the track, at most four minutes, 15 seconds when the length is unknown): nobody plays a track twice that fast, so it comes from a second scrobbler or a play sent twice.

Every client gets its own token, which belongs to one profile. Tokens are created on the command line for now (the binary is `musicbanana`, or `cargo run --release --` in `backend/`):

```sh
musicbanana token create --user <username> --label Navidrome                    # prints the token, only this once
musicbanana token create --user <username> --profile <slug> --label "Work laptop"
musicbanana token list [--user <username>]
musicbanana token revoke <id>
```

### Navidrome, and apps that play from it (Supersonic, Ultrasonic, …)

Subsonic apps report plays to Navidrome, and Navidrome passes them on to ListenBrainz, so pointing Navidrome at musicbanana covers all of them:

1. Set Navidrome's ListenBrainz address to musicbanana and restart Navidrome, in `navidrome.toml`:
   ```toml
   ListenBrainz.BaseURL = "https://musicbanana.example.org/api/listenbrainz/1/"
   ```
   or as the environment variable `ND_LISTENBRAINZ_BASEURL`. This holds for every user of that Navidrome: they can scrobble to musicbanana (each with their own token), but no longer to listenbrainz.org. Last.fm scrobbling keeps working.
2. In Navidrome's personal settings, turn on "Scrobble to ListenBrainz" and paste the token. Navidrome checks it right away.
3. Leave scrobbling switched on in the apps. "Now playing" shows on the profile page while a track plays.

Tested with Navidrome 0.64.2: linking the token, "now playing" and listens sent through its Subsonic `scrobble` endpoint.

### Other ListenBrainz clients

Clients that let you change the ListenBrainz server want either the API root `https://musicbanana.example.org/api/listenbrainz/1/` or the server `https://musicbanana.example.org/api/listenbrainz`, plus the token.

### Artists and albums of the same name

Listens of files tagged with MusicBrainz Picard come with MusicBrainz IDs, which Navidrome passes on. musicbanana uses those of the artist (for a track by one artist), of the album's release group and of the recording:

- Artists or albums of the same name with different IDs stay apart, such as two bands called Nirvana or two albums called "Weezer". The first ID that comes with a name goes to the entry listened to so far, imported listens included, and listens without an ID keep counting for that one.
- An album ID finds its album under any title, so "Geräusch (Deluxe)" counts for "Geräusch" when both belong to the same release group, and that title does from then on without an ID too. Likewise for a recording ID and its track.
- Tracks of the same title by one artist stay one track whatever their IDs, since MusicBrainz has recordings of their own for a live version or an edit.
- An artist ID that came with another name before is not used: a client may send the ID of A along with "A & B", and then the listens of A must not count for "A & B".

The pages of an artist, album or track link to MusicBrainz.

## Merging duplicates

The same artist, album or track can end up in the catalog under several spellings: "Die Aerzte" next to "Die Ärzte", "Bjork" next to "Björk", typos, or "Unrockbar (Live)" next to "Unrockbar". `merge suggest` lists look-alikes, each line with the command that merges them:

```sh
musicbanana merge suggest                     # artists, releases and recordings, 30 each
musicbanana merge suggest artists --limit 100
musicbanana merge artist 977 30 --dry-run     # what merging artist 977 into 30 would change
musicbanana merge artist 977 30
```

With Docker, prefix them with `docker compose exec musicbanana`. A suggestion says why:

- **same letters:** only case, accents, punctuation, spaces, a leading "The" or "&" for "and" differ;
- **one letter apart:** a letter more, missing, different or swapped with its neighbour, in names of six letters or more and never where digits differ ("Chapter 1", "Chapter 2");
- **version:** the same title apart from a note in brackets, after a dash or "feat.", so possibly a live version or a remaster; these come last.

The entry with fewer listens goes into the one with more, a version into the plain title. Releases and recordings are compared within one artist only, so merge artists first: merging an artist also merges its releases and recordings into the other artist's ones with the same title and moves the rest over. A merge points the listens and the spellings at the remaining entry, so later scrobbles with the old spelling land there too; the raw strings of the listens stay as they were. There is no undo yet, hence `--dry-run`.

Two artists or albums that both have a MusicBrainz ID are neither suggested nor merged, as their IDs say they are different ones of the same name; merging an artist moves its albums over instead of merging them with one of another ID. A merge takes the IDs along, so listens with the ID of the merged entry count for the remaining one.

## Importing the old musicbanana-php database

The importer reads the `mb_*` tables straight from MySQL/MariaDB and writes into an empty musicbanana database (it refuses to run twice). It repairs double-encoded names, follows old merges and wraps the old MD5 password hashes in argon2id; details in [docs/design.md](docs/design.md).

```sh
cd backend
DATABASE_URL=postgres://postgres@127.0.0.1:5432/musicbanana \
  cargo run --release -- import-php --from mysql://root@127.0.0.1:3306/musicbanana
```

It prints a summary (accounts, artists, listens, skipped rows, repaired names). Afterwards the history is at `/u/<old username>`. The import test (`tests/import_php.rs`) runs against a MySQL server named by `LEGACY_MYSQL_URL` and is skipped without it.

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

Afterwards the web interface is at `http://<server>:3000`; `MUSICBANANA_PORT` in `.env` changes the port. Commands of the binary run inside the container, for example the token for Navidrome:

```sh
docker compose exec musicbanana musicbanana token create --user <username> --label Navidrome
```

Navidrome needs the address under which its container reaches musicbanana, as `ND_LISTENBRAINZ_BASEURL` in its environment:

- through the published port, with the server's address in your network: `http://192.168.1.10:3000/api/listenbrainz/1/` (not `localhost`, which inside the Navidrome container is the container itself);
- or, when both containers share a Docker network (for example with the Navidrome service in the same compose file), by service name: `http://musicbanana:3000/api/listenbrainz/1/`.

Then restart Navidrome and link the token as described [above](#navidrome-and-apps-that-play-from-it-supersonic-ultrasonic-).

**Moving existing data in**, for example the database the PHP import went into:

```sh
pg_dump -Fc -d postgres://postgres@127.0.0.1:5432/musicbanana -f musicbanana.dump   # where the data is now
docker compose stop musicbanana
docker compose exec -T db pg_restore -U musicbanana -d musicbanana --clean --if-exists --no-owner < musicbanana.dump
docker compose start musicbanana
```

**Updates:** `docker compose pull && docker compose up -d`, or `git pull && docker compose up -d --build` to build it yourself. Migrations run when the server starts.

**Backups:** `docker compose exec -T db pg_dump -U musicbanana -Fc musicbanana > musicbanana-$(date +%F).dump`, restored as above.

**Logs:** `docker compose logs -f musicbanana`.

## Without Docker

```sh
cd frontend && pnpm install --frozen-lockfile && pnpm build
cd backend && cargo build --release
STATIC_DIR=../frontend/build DATABASE_URL=postgres://… ./target/release/musicbanana
```

Environment: `DATABASE_URL` (required), `LISTEN_ADDR` (default `127.0.0.1:3000`), `STATIC_DIR` (default `../frontend/build`), `RUST_LOG`.
