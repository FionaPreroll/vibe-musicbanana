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
| `/u/<username>` (default profile) or `/u/<username>/<slug>`: listens per year, top artists, albums and tracks, recent listens, what is playing now; `?year=2012` narrows everything to one year | `GET /api/profiles/<username>/<slug>?tz=`, `…/top/{artists,releases,recordings}?year=&tz=&limit=`, `…/listens?before=&limit=`, `…/now-playing` |

Only public profiles are served until there is a login. A year starts at midnight in `tz` (an IANA name such as `Europe/Berlin`, UTC by default); the frontend sends the browser's time zone. `listens` pages backwards: pass a page's `next` as `before`. `now-playing` is `null` when nothing plays; the open page asks again every 30 seconds and adds new listens on top.

## Scrobbling

musicbanana speaks the part of the [ListenBrainz API](https://listenbrainz.readthedocs.io/en/latest/users/api/core.html) that players use to scrobble, under `/api/listenbrainz/1/`: `POST submit-listens` (`single`, `import` and `playing_now`) and `GET validate-token`. Limits, checks and error responses follow listenbrainz-server. A listen keeps the strings and extra data (`additional_info`) exactly as sent and is matched to the catalog through the alias tables; unknown artists, albums and tracks are created. A second listen at the same second is skipped, so clients can safely resend.

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

## Importing the old musicbanana-php database

The importer reads the `mb_*` tables straight from MySQL/MariaDB and writes into an empty musicbanana database (it refuses to run twice). It repairs double-encoded names, follows old merges and wraps the old MD5 password hashes in argon2id; details in [docs/design.md](docs/design.md).

```sh
cd backend
DATABASE_URL=postgres://postgres@127.0.0.1:5432/musicbanana \
  cargo run --release -- import-php --from mysql://root@127.0.0.1:3306/musicbanana
```

It prints a summary (accounts, artists, listens, skipped rows, repaired names). Afterwards the history is at `/u/<old username>`. The import test (`tests/import_php.rs`) runs against a MySQL server named by `LEGACY_MYSQL_URL` and is skipped without it.

## Production-ish

```sh
cd frontend && pnpm install --frozen-lockfile && pnpm build
cd backend && cargo build --release
STATIC_DIR=../frontend/build DATABASE_URL=postgres://… ./target/release/musicbanana
```

Environment: `DATABASE_URL` (required), `LISTEN_ADDR` (default `127.0.0.1:3000`), `STATIC_DIR` (default `../frontend/build`), `RUST_LOG`.
