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

Requirements: Rust (stable), Node 22+, Docker (or any PostgreSQL 16+).

```sh
docker compose up -d db                       # PostgreSQL on localhost:5432
cp backend/.env.example backend/.env

cd backend && cargo run                       # applies migrations, API on http://127.0.0.1:3000
cd frontend && npm install && npm run dev     # UI on http://localhost:5173, proxies /api
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
cd frontend && npm run lint && npm run check && npm run build
```

## Importing the old musicbanana-php database

The importer reads the `mb_*` tables straight from MySQL/MariaDB and writes into an empty musicbanana database (it refuses to run twice). It repairs double-encoded names, follows old merges and wraps the old MD5 password hashes in argon2id; details in [docs/design.md](docs/design.md).

```sh
cd backend
DATABASE_URL=postgres://postgres@127.0.0.1:5432/musicbanana \
  cargo run --release -- import-php --from mysql://root@127.0.0.1:3306/musicbanana
```

It prints a summary (accounts, artists, listens, skipped rows, repaired names). The import test (`tests/import_php.rs`) runs against a MySQL server named by `LEGACY_MYSQL_URL` and is skipped without it.

## Production-ish

```sh
cd frontend && npm ci && npm run build
cd backend && cargo build --release
STATIC_DIR=../frontend/build DATABASE_URL=postgres://… ./target/release/musicbanana
```

Environment: `DATABASE_URL` (required), `LISTEN_ADDR` (default `127.0.0.1:3000`), `STATIC_DIR` (default `../frontend/build`), `RUST_LOG`.
