# Development

[Back to the README](../README.md)

- [Layout](#layout)
- [Setting up](#setting-up)
- [Changing SQL](#changing-sql)
- [Checks](#checks)
- [Tests](#tests)
- [Coverage](#coverage)
- [Texts and translations](#texts-and-translations)

Design notes and decisions, with the data model: [design.md](design.md) (German).

## Layout

| Path | What |
|---|---|
| `backend/` | Rust: Axum HTTP server, sqlx + PostgreSQL. Serves `/api/*` and the built frontend; the same binary has the command line (`musicbanana account …`, `merge …`, `import-php` and so on). |
| `backend/migrations/` | SQL migrations, applied automatically on startup. |
| `backend/.sqlx/` | Offline query cache so the crate builds without a database (`cargo sqlx prepare`). |
| `backend/tests/` | Integration tests against PostgreSQL, with fixtures in `tests/fixtures/`. |
| `frontend/` | SvelteKit + TypeScript + Tailwind, built as a static SPA (`adapter-static`). |
| `frontend/src/lib/i18n/` | The English and German texts of the interface. |
| `frontend/src/test/` | Helpers for the frontend tests. |
| `Dockerfile`, `deploy/` | The image with server and frontend, and a compose file to run it with PostgreSQL ([Installation](installation.md)). |
| `docs/` | These docs. |

## Setting up

Requirements: Rust (stable), Node 22+, pnpm (e.g. `npm install -g pnpm`; it switches to the version pinned in `frontend/package.json` by itself), Docker (or any PostgreSQL 16+). Node and pnpm are only needed to build the frontend; the server runs without them.

```sh
docker compose up -d db                       # PostgreSQL on localhost:5432
cp backend/.env.example backend/.env

cd backend && cargo run                       # applies migrations, API on http://127.0.0.1:3000
cd frontend && pnpm install && pnpm dev       # UI on http://localhost:5173, proxies /api
```

`cargo run -- <command>` runs the command line, e.g. `cargo run -- account create fiona --email fiona@example.org`.

## Changing SQL

sqlx checks every `query!` at compile time. Builds use the committed query cache in `backend/.sqlx/` (`SQLX_OFFLINE=true` in `.cargo/config.toml`), so they work without a database. After adding or changing a query or a migration, refresh the cache against a migrated database and commit the updated `.sqlx/` files:

```sh
cargo install sqlx-cli --version '~0.9' --no-default-features --features postgres,rustls   # once
cd backend && cargo sqlx migrate run && cargo sqlx prepare
```

Until then the build fails with "no cached data for this query". To check queries live while editing, run with `SQLX_OFFLINE=false` against a migrated database.

A new migration takes the next free number in `backend/migrations/`; with several branches open at once, check the others too, as two migrations of the same number can't both go in.

## Checks

The same ones CI runs:

```sh
cd backend && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd frontend && pnpm lint && pnpm check && pnpm test && pnpm build
```

## Tests

**Backend:** unit tests sit next to the code, integration tests in `backend/tests/`. These use `#[sqlx::test]`, which makes a fresh database for each test on the server in `DATABASE_URL` (the one from `docker compose up -d db` will do), so they need PostgreSQL but never touch your data. The PHP import test runs only with `LEGACY_MYSQL_URL` set, see [Importing the PHP database](import-php.md). Outside services (YourSpotify, the Cover Art Archive, MusicBrainz) are replaced by stand-ins, so the tests need no network. There are no fuzz or property tests so far.

**Frontend:** the tests (`*.test.ts` next to what they test) run with [Vitest](https://vitest.dev) in [happy-dom](https://github.com/capricorn86/happy-dom), components and pages through [Testing Library](https://testing-library.com/docs/svelte-testing-library/intro). They need no backend: `src/test/` has a fake API (`fakeApi`), stand-ins for `$app/state` and `$app/navigation`, and sample data. Tests run in Europe/Berlin and US English whatever the machine says, so dates and numbers come out the same everywhere; they check the English texts, so changing a text means changing the tests that expect it. `pnpm test:watch` reruns them on every change.

## Coverage

CI runs the backend tests through [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) and the frontend tests with `pnpm test:coverage`, and uploads both to [Codecov](https://codecov.io/gh/FionaPreroll/vibe-musicbanana) (secret `CODECOV_TOKEN`) under the flags `backend` and `frontend`. Coverage never fails a check (`codecov.yml`).

The same reports locally:

```sh
cargo install cargo-llvm-cov && cd backend && cargo llvm-cov --html   # then target/llvm-cov/html/index.html
cd frontend && pnpm test:coverage                                     # then coverage/lcov-report/index.html
```

## Texts and translations

The interface speaks English and German. Every text lives in `frontend/src/lib/i18n/`: `en.ts` sets the shape, `de.ts` follows it, and a test checks that both have the same entries. Components use `m.<section>.<key>`; texts with values are functions, and plurals, lists and sentences with links have helpers there (`plural()`, `formatList`, `<Fill>`).

- **A new text** goes into `en.ts` and `de.ts`, never straight into a component. The German texts use "du".
- **The backend's messages** stay English. `server-errors.ts` maps the ones users see to the catalog; a new error users can run into needs a pattern there and an entry in both languages.
- **A new language:** copy `de.ts`, add its code to `languages` in `locale.svelte.ts` and to `catalogs` in `index.ts`. The account's choice (`account.language`) takes any two- or three-letter code; an account's language the interface lacks counts as no choice, and then the first browser language it speaks, or else English, is used.
