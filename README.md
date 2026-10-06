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
| `/` lists the profiles the viewer may see | `GET /api/profiles` |
| `/u/<username>` (default profile) or `/u/<username>/<slug>`: listens per year, top artists, albums and tracks, the top artists of each year, recent listens, what is playing now; `?days=30` (also 7, 90, 365), `?year=2012` or `?from=2009-06-01&to=2009-08-31` narrows the top lists and listens to that period | `GET /api/profiles/<username>/<slug>?tz=`, `…/top/{artists,releases,recordings}?year=&from=&to=&tz=&limit=`, `…/top/artists/years?tz=&limit=`, `…/listens?before=&limit=`, `…/now-playing`, `…/live` |
| `/u/<username>` without a period starts with "This week" (Monday to Sunday, or from the first weekday picked in the settings): listens against the week before up to the same weekday and time, days with listens, the current and the longest streak of days in a row with listens (left out when the viewer or the profile's owner turned streaks off in the settings), listens per day of both weeks and the five artists of the week, marked when heard for the first time; left out when the profile had no listens this week or last and no running streak | `GET /api/profiles/<username>/<slug>/week?day=&tz=&week_start=` (`day` picks another week, today by default; `week_start` is 1 for Monday, the default, to 7 for Sunday) |
| Below it, "On this day": today's date in each earlier year with listens on it, the latest first (left out when there is none), each with its listens, three most heard artists and five most heard tracks, linking to the profile page for that day; 29 February looks back to the 28th in other years | `GET /api/profiles/<username>/<slug>/on-this-day?day=&tz=&source=` (`day` looks back from another date, today by default) |
| Further down, "Lost & found": the artists and tracks the profile heard a lot (at least 20 listens for an artist, 10 for a track) and not in the year before its latest listen, with their listens and years. The order weighs both: listens times 1 − e^(−years gone / 2), 39 % a year after the last listen, 63 % after two, 86 % after four, so between one and three years gone counts a lot and ten or fifteen hardly differ; a source narrows only what counts as a lot. The owner can dismiss one they don't want back (an artist takes its tracks along), take it back right away, or show it again later from the list of dismissed ones; dismissals belong to the profile and follow merges | `GET /api/profiles/<username>/<slug>/lost-and-found?limit=&source=`, `GET`/`POST /api/me/profiles/<slug>/lost-and-found/hidden` (`{"kind": "artist"\|"recording", "id"}`), `DELETE /api/me/profiles/<slug>/lost-and-found/hidden/<kind>/<id>` |
| `/u/<username>/year/2016` (after the slug for other profiles), linked from the year view of the profile page: the year in review, with listens against the year before, artists and how many were new, days with listens, the busiest month and day, listens per month of both years, the top artists, albums and tracks, the most heard artists heard for the first time, the biggest risers (artists heard before with the most listens more than in the year before, and their places in both years' charts) and the longest listening session (it goes on while the next listen starts at most 30 minutes after the end of the one before); a running year counts up to now and is compared with the year before up to the same day and time | `GET /api/profiles/<username>/<slug>/year/<year>?tz=` |
| `/u/<username>/artist/<id>-<name>`, `…/album/…` and `…/track/…` (after the slug for other profiles), e.g. `/u/fiona/artist/1-die-arzte`: listens per month, first and last listen, phases of heavy listening, the albums and tracks heard | `GET /api/profiles/<username>/<slug>/{artists,releases,recordings}/<id>?tz=` |
| `/u/<username>/search?q=` (after the slug for other profiles), also from the search field in the header of a profile's pages (`/` jumps into it): the artists, albums and tracks the profile has heard whose names hold every word, ignoring case and accents, the most heard first; a word may also be the artist's name of an album or track ("ärzte unrockbar") | `GET /api/profiles/<username>/<slug>/search?q=&limit=` |
| `?source=Navidrome` on any page of a profile (overview, artist, album, track, search), picked under the period: only the listens from that source; the sources are the clients without their version ("Navidrome 0.64.2 (…)" is "Navidrome"), "Spotify via YourSpotify" and the imports such as `import:php-2016` | every API route of a profile takes `source=`; `GET /api/profiles/<username>/<slug>/sources` lists them with their listens |
| `/login` | `POST /api/session` (`{"login", "password"}`), `DELETE /api/session` logs out |
| `/settings`: the account's profiles (create, rename, who sees them), scrobble tokens (create, revoke), YourSpotify connections (connect, remove), time zone and first day of the week, whether to show streaks, user name and password | `GET /api/me`, `POST /api/me/profiles`, `PATCH /api/me/profiles/<slug>`, `GET`/`POST /api/me/tokens`, `DELETE /api/me/tokens/<id>`, `GET`/`POST /api/me/yourspotify`, `DELETE /api/me/yourspotify/<id>`, `PUT /api/me/time` (`{"time_zone", "week_start"}`; `time_zone` null for the browser's, `week_start` 1 for Monday to 7 for Sunday), `PUT /api/me/streaks` (`{"show_streaks"}`), `PUT /api/me/username`, `PUT /api/me/password`; `GET /api/renamed/<old name>/<slug>` gives the current name |
| `/merges` (admins only): merge suggestions of artists, albums and tracks, merging with a dry run first, merging by hand, hidden suggestions and the merge log with undo, see [Merging duplicates](#in-the-browser) | `GET /api/admin/merges/suggestions/<artist\|release\|recording>?limit=`, `GET`/`POST /api/admin/merges/hidden/<kind>` (`{"from", "into"}`), `DELETE /api/admin/merges/hidden/<kind>/<from>/<into>`, `GET /api/admin/catalog/<kind>?q=`, `POST /api/admin/merges` (`{"kind", "from", "into", "dry_run", "force"}`), `GET /api/admin/merges?before=&limit=`, `POST /api/admin/merges/<n>/undo` (`{"dry_run"}`) |

A profile is public, for followers (its owner and the accounts it let follow) or private (its owner only); to anybody else it does not exist (404), in the list and on every page and API route below it. Years and days start at midnight in `tz` (an IANA name such as `Europe/Berlin`, UTC by default); the frontend sends the time zone picked in the settings, or the browser's when there is none (or when nobody is logged in). `from` and `to` are the first and last day of a period, both included, and either can be left out; `year=2012` is short for the whole year. `listens` pages backwards: pass a page's `next` as `before`. `now-playing` is `null` when nothing plays. `live` is a stream of Server-Sent Events (`now-playing`, `listens`) that tells the open page when either changed, so it shows a new track or listen right away and adds new listens on top; scrobbles from other processes, such as `import-yourspotify --every`, arrive too (PostgreSQL `NOTIFY`). A reverse proxy must not buffer it; nginx leaves it alone because of the `X-Accel-Buffering: no` header. Only the id in the address of an artist, album or track counts; the name after it is for the reader, and the page moves to the current one when it differs, as after a rename or for links with the id alone. The months of an artist, album or track run from the profile's first listen to its last, leaving out a year or more without any listens (shown as a break); an entry that was merged into another one answers with that one.

Every page comes light or dark, the dark one in black and banana yellow. It follows the system setting until the moon or sun at the top picks the other one; the browser remembers that pick, and picking what the system shows follows the system again.

## Accounts and logging in

The accounts of the old musicbanana log in with their old passwords; the first login replaces the old MD5 hash with a proper one. New accounts, and new passwords for forgotten ones, come from the command line (the binary is `musicbanana`, or `cargo run --release --` in `backend/`):

```sh
musicbanana account create <username> --email <address>   # asks for the password, with a default profile
musicbanana account password <username>                     # asks for the new password
musicbanana account rename <username> <new name>
musicbanana account email <username> <address>
musicbanana account admin <username> [--off]               # the status and merges pages, see below
```

Both read the password from standard input when that is not a terminal, e.g. `echo "$PASSWORD" | musicbanana account password fiona`. Passwords need at least 8 characters.

A user name has up to 32 letters, digits, dashes, dots and underscores and starts with a letter or digit. It can be changed on the settings page or with `account rename`. Links with the old name lead to the new one, as long as the viewer may see the profile, and no other account can take the old name; renaming back to it is fine. Scrobble tokens and YourSpotify connections stay as they are.

The email address logs in too and can be changed on the settings page (with the password) or with `account email`. musicbanana sends no mail, so nobody checks that it works.

A further profile, for example one per player or for a second person's listens, comes from the settings page or from

```sh
musicbanana profile create --user <username> <slug> --name "Spotify" --visibility private   # public (default), followers or private
```

Logged-in accounts can follow a profile with the button next to its name; the start page lists the profiles they follow. A public profile is followed right away. A profile for followers shows strangers a page to ask to follow instead (anonymous visitors still get a 404), and the owner says yes or no under Followers in the settings, where followers can be removed again. Making a profile public answers the open requests with yes. Follows from the old musicbanana ("friends") count as accepted.

Wrong listens (a scrobbler that ran twice, a party on your account, a bad import) are taken out on the page "Edit listening history", linked from each profile in the settings. It lists the profile's listens, narrowed by words (artist, track or album, ignoring case and accents), a time span and a source, and moves the ones picked, or all that match, to the trash. Listens in the trash leave the charts and every page, and a player or an import sending them again doesn't bring them back. From the trash they can be restored, with an undo right after moving them, or deleted for good.

Under "Your data" in the settings (`GET /api/me/export`), users download everything musicbanana keeps about their account as one JSON file: the account with its former user names, the profiles with all their listens and their trash, follows both ways, scrobble tokens, YourSpotify connections and current logins. Each listen is in ListenBrainz's import format (`listened_at`, `track_metadata` with the names as the player sent them) plus what musicbanana made of it, so the listens can move to another service. The password hash, the tokens themselves and login cookies stay out. The file is written while it downloads, so big histories need no extra memory.

Users can't delete profiles or their account themselves; the settings page tells them to ask the admin, who runs

```sh
musicbanana profile delete --user <username> <slug>   # a profile other than the default one, with its listens
musicbanana account delete <username>                 # the account with all its profiles and listens
```

Without `--yes` both only list what would go (profiles, listens, listens in the trash, scrobble tokens, YourSpotify connections, followers) and delete nothing. Artists, albums and tracks stay in the shared catalog. The deleted listens are also taken out of the journal of merges, so `merge undo` can't bring them back, and the user name is free again. There is no undo: back up the database first if in doubt.

A login lasts 30 days after the last visit. Changing the password logs out the account's other browsers. After 10 wrong passwords for a name within 15 minutes, that name is refused for the rest of the 15 minutes. The login cookie is `HttpOnly` and `SameSite=Lax`, and `Secure` when a reverse proxy in front sends `X-Forwarded-Proto: https`, which Caddy, Traefik and nginx (with `proxy_set_header X-Forwarded-Proto $scheme`) do. Requests that change something only take JSON, so other sites can't send them with the cookie.

## Scrobbling

musicbanana speaks the part of the [ListenBrainz API](https://listenbrainz.readthedocs.io/en/latest/users/api/core.html) that players use to scrobble, under `/api/listenbrainz/1/`: `POST submit-listens` (`single`, `import` and `playing_now`) and `GET validate-token`. Limits, checks and error responses follow listenbrainz-server. A listen keeps the strings and extra data (`additional_info`) exactly as sent and is matched to the catalog through the alias tables; unknown artists, albums and tracks are created. A second listen at the same second is skipped, so clients can safely resend. So is a listen of the same track that follows another one sooner than a player counts a play (half the track, at most four minutes, 15 seconds when the length is unknown): nobody plays a track twice that fast, so it comes from a second scrobbler or a play sent twice.

Every client gets its own token, which belongs to one profile. Tokens are created on the settings page, or on the command line:

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

### Artists, albums and tracks of the same name

Listens of files tagged with MusicBrainz Picard come with MusicBrainz IDs, which Navidrome passes on. musicbanana uses those of the artist (for a track by one artist), of the album's release group and of the recording:

- Artists, albums or tracks of the same name with different IDs stay apart, such as two bands called Nirvana, two albums called "Weezer", or a song and its live version. The first ID that comes with a name goes to the entry listened to so far, imported listens included, and listens without an ID keep counting for that one.
- A track ID is that of the MusicBrainz recording, the same audio on whatever album or single. A remaster mostly has the ID of the original and counts for the same track; a live version or a new recording has one of its own.
- An album ID finds its album under any title, so "Geräusch (Deluxe)" counts for "Geräusch" when both belong to the same release group, and that title does from then on without an ID too. Likewise for a recording ID and its track.
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
- **version:** the same title apart from a note in brackets, after a dash or "feat.", so possibly a live version; these come last.

The entry with fewer listens goes into the one with more, a version into the plain title. Releases and recordings are compared within one artist only, so merge artists first: merging an artist also merges its releases and recordings into the other artist's ones with the same title and moves the rest over. A merge points the listens and the spellings at the remaining entry, so later scrobbles with the old spelling land there too; the raw strings of the listens stay as they were.

Every merge can be taken back. A merge prints its number, and `merge log` lists them:

```sh
musicbanana merge log                  # the latest 30, newest first
musicbanana merge undo 12 --dry-run    # what undoing merge 12 would put back
musicbanana merge undo 12
```

Undoing puts back what the merge changed: the listens, spellings and MusicBrainz IDs go back to the merged entry, which stands on its own again, and an artist gets its albums and tracks back. What changed since stays as it is now, such as listens that came in under the merged spelling after the merge, and the command says how many such rows it left. A merge that a later one built on (merging the remaining entry on into a third one, say) can only be undone after that later one.

### In the browser

Admins (see [Status page](#status-page)) have a Merges link at the top that leads to `/merges`, which does the same as the commands: the suggestions of artists, albums and tracks, each with a preview of what merging would change (the dry run) before it merges, the other way round, or hidden for good; a merge by hand of any two entries, found by name or `#id`; and the log of all merges, also those from the command line, each with a preview before it is undone. A hidden suggestion (a live version that should stay apart, say) leaves `merge suggest` too; the page lists the hidden ones and can suggest them again. Merges and undos from the page go into the log under `musicbanana::merge`, with the account that made them.

Two entries that both have a MusicBrainz ID are not suggested, as their IDs say they are different ones of the same name, and merging them takes `--force`: for an artist's other name that should count for the main one, or a recording MusicBrainz lists twice. Merging an artist moves its albums and tracks over instead of merging them with one of another ID. A merge takes the IDs along, so listens with the ID of the merged entry count for the remaining one.

### Remasters and deluxe editions

Streaming services name the same track and album after the edition it comes from: "Help! - Remastered 2009", "Song 2 - 2012 Remaster", "Rumours (Super Deluxe Edition)", "Album [Remastered]". musicbanana leaves such notes out when it matches a listen to the catalog, so they count for "Help!" and "Rumours"; the listen itself keeps the title as it was sent. A note counts as an edition when it is made of words like remaster(ed), deluxe, expanded, anniversary, edition, bonus track, version and years ("2009", "25th"); notes of versions that sound different, such as "Live", "Radio Edit", "Acoustic" or "Mono", stay.

Albums and tracks that came in before (for example from a YourSpotify import made with an older musicbanana) are tidied with one command:

```sh
musicbanana merge editions --dry-run   # one line per album and track it would change
musicbanana merge editions
```

Each album or track with an edition note is merged into the one of its plain title, or renamed to the plain title where there is none yet. One whose plain title belongs to an entry with other MusicBrainz IDs is kept, as the IDs say they are different. Running it again is harmless. Each merge it makes shows up in `merge log` and can be taken back with `merge undo`; renaming is not undone, but does no harm, as listens with the old title still find the entry.

## Renaming and MusicBrainz IDs by hand

```sh
musicbanana rename artist 12 "Nirvana (UK)"
musicbanana mbid add artist 12 https://musicbrainz.org/artist/<id>
musicbanana mbid remove recording 345 <id>
```

The number is the entry's id, as in the address of its page (`/u/<name>/artist/12`) or shown by `merge suggest`; with Docker, prefix the commands with `docker compose exec musicbanana`.

A rename keeps the old spellings leading to the entry, so later listens under the old name still count for it, and the new name becomes a spelling too unless it leads to another entry already. Renaming one of two artists of the same name, say to "Nirvana (UK)", tells them apart in the charts; its MusicBrainz ID keeps counting under the old name.

`mbid add` gives an entry a MusicBrainz ID, given as such or as the address of its MusicBrainz page; an album (release) takes the ID of its release group. Listens with that ID count for the entry from then on, while the listens so far stay where they are. `mbid remove` takes an ID away again, such as the ID of a live version that came first and went to the track with all the old listens: the next listen with the ID is then matched like one with a new ID, and `merge` can join the studio version into the old track.

## Importing the old musicbanana-php database

The importer reads the `mb_*` tables straight from MySQL/MariaDB and writes into an empty musicbanana database (it refuses to run twice). It repairs double-encoded names, follows old merges and wraps the old MD5 password hashes in argon2id; details in [docs/design.md](docs/design.md).

```sh
cd backend
DATABASE_URL=postgres://postgres@127.0.0.1:5432/musicbanana \
  cargo run --release -- import-php --from mysql://root@127.0.0.1:3306/musicbanana
```

It prints a summary (accounts, artists, listens, skipped rows, repaired names). Afterwards the history is at `/u/<old username>`. The import test (`tests/import_php.rs`) runs against a MySQL server named by `LEGACY_MYSQL_URL` and is skipped without it.

## Spotify plays from YourSpotify

[YourSpotify](https://github.com/Yooooomi/your_spotify) keeps the history of a Spotify account: the plays from Spotify's data export and those it has fetched from Spotify since. musicbanana takes them from there into a profile:

```sh
export YOURSPOTIFY_TOKEN=…    # the public token from YourSpotify's settings
musicbanana import-yourspotify --from http://192.168.1.10:8080 --user <username>
```

`--from` is the address of YourSpotify's API (what YourSpotify has as `API_ENDPOINT`), not of its web interface. The public token gives read access to all statistics of the account, so keep it like a password; set as `YOURSPOTIFY_TOKEN` it stays out of the shell history, `--token` works too.

The first run fetches the whole history, oldest first, a month at a time, and stores each month before fetching the next, so the plays show up in the profile while it runs; a run that stops halfway goes on from there the next time. Later runs fetch only the plays after the latest one imported. `--all` fetches everything again, for example after YourSpotify has imported an older Spotify export; plays the profile has already are skipped, as with scrobbles. The plays go to the default profile, `--profile <slug>` picks another one, and `--every 15m` keeps the command running and fetches the new plays every 15 minutes (or `90s`, `1h`).

A play counts for its first artist, with track and album titled as on Spotify; the Spotify IDs and all artists stay in the listen's extra data. Spotify gives no MusicBrainz IDs. Its titles often carry an edition like "(Deluxe Edition)" or "- Remastered 2011", which the catalog leaves out (see [Remasters and deluxe editions](#remasters-and-deluxe-editions); plays imported before that need `merge editions` once). Other additions such as "- Live" stay, and `merge suggest` finds those next to the albums and tracks from other players.

### Connections the server keeps up to date

Instead of running the importer yourself, a profile can be connected to a YourSpotify: on the settings page under "Spotify via YourSpotify" (profile, API address, public token), or with

```sh
export YOURSPOTIFY_TOKEN=…
musicbanana yourspotify add --from http://192.168.1.10:8080 --user <username> --profile <slug>
musicbanana yourspotify list                 # all connections with their state
musicbanana yourspotify remove <id>
```

The token is tried before the connection is kept. From then on `musicbanana serve` imports the whole history once and afterwards the new plays every 15 minutes, as the importer above would; the settings page shows when it last finished, how many plays it brought in so far and the last error. Each profile takes one connection, so two Spotify accounts go to two profiles, of the same account or of different ones. The token is stored in the database and never shown again, logged or sent to the browser; removing the connection deletes it. The settings page only takes the addresses in `YOURSPOTIFY_ALLOWED_URLS` (separated by commas, e.g. `http://192.168.1.10:8080`; an address also allows the paths below it, with the same scheme, host and port), so that nobody can make the server fetch from elsewhere in your network; without it the page takes none, and the command line takes any address. The server does not follow redirects from YourSpotify. Connections made before an address left the list keep running; `yourspotify list` shows them and `yourspotify remove` stops them.

YourSpotify has no documented API; the importer uses the route its web interface reads the history from (`GET /spotify/gethistory`), which a new YourSpotify version could change. A YourSpotify that takes no time range there gets the old way: everything fetched newest first, then stored at once.

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

After logging in, the settings page makes the token for Navidrome.

Navidrome needs the address under which its container reaches musicbanana, as `ND_LISTENBRAINZ_BASEURL` in its environment:

- through the published port, with the server's address in your network: `http://192.168.1.10:3000/api/listenbrainz/1/` (not `localhost`, which inside the Navidrome container is the container itself);
- or, when both containers share a Docker network (for example with the Navidrome service in the same compose file), by service name: `http://musicbanana:3000/api/listenbrainz/1/`.

Then restart Navidrome and link the token as described [above](#navidrome-and-apps-that-play-from-it-supersonic-ultrasonic-).

**Spotify plays from YourSpotify** (see [above](#spotify-plays-from-yourspotify)): the simplest way is a [connection](#connections-the-server-keeps-up-to-date) on the settings page, which the `musicbanana` container keeps up to date by itself. The older way still works: with these lines in `.env`, `docker compose up -d` also starts the service `yourspotify`, which imports the history once and then the new plays every 15 minutes (`YOURSPOTIFY_EVERY` changes that). The address is the one under which the container reaches YourSpotify's API, as for Navidrome not `localhost`. `docker compose logs yourspotify` shows what it imported.

```sh
COMPOSE_PROFILES=yourspotify
YOURSPOTIFY_URL=http://192.168.1.10:8080
YOURSPOTIFY_TOKEN=<public token>
YOURSPOTIFY_USER=<username>
```

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

Environment: `DATABASE_URL` (required), `LISTEN_ADDR` (default `127.0.0.1:3000`), `STATIC_DIR` (default `../frontend/build`), `ACCESS_LOG`, `RUST_LOG` (see [Logs](#logs)), `YOURSPOTIFY_ALLOWED_URLS` (see [above](#connections-the-server-keeps-up-to-date)).

## Logs

The server logs to standard output (`docker compose logs -f musicbanana`). Besides start-up and errors:

- `musicbanana::access`: one line per request, with the client (the first `X-Forwarded-For` address behind a reverse proxy), method, path with query, status and milliseconds, e.g. `192.168.1.20 POST /api/me/profiles 201 12 ms`. Health checks and the frontend's files are left out. `ACCESS_LOG=off` switches it off.
- `musicbanana::account`: logins (failed ones as warnings, with the name tried), logouts and every change in the settings: profiles, scrobble tokens, YourSpotify connections (never their token), user name and password.
- `musicbanana::merge`: merges, undos and hidden suggestions from the page Merges, with the admin's account id.
- `musicbanana::connections`: each round of a YourSpotify connection, when it starts and what it brought, or why it failed; the import's progress comes from `musicbanana::yourspotify`.

`RUST_LOG` picks what is logged, by target and level (default `musicbanana=info,tower_http=info`): `RUST_LOG=musicbanana=info,musicbanana::account=warn` keeps only failed logins of the settings, `RUST_LOG=musicbanana=debug` logs more, and `musicbanana::access=off` is the same as `ACCESS_LOG=off`.

## Status page

Admins have a Status link at the top that leads to `/status`: the build (version and commit), the server's CPU and memory, the machine's load and memory, the database (size, migration, connections, cache hits, rows and size per table, listens of the last 24 hours by source), the YourSpotify connections and how long each API route took since the server started (mean, median, 95th percentile, maximum and 5xx errors). It refreshes every 10 seconds. `musicbanana account admin <username>` makes an account an admin, `--off` takes that back; nobody else sees the page, nor the page [Merges](#in-the-browser).

The commit comes from the image build (`--build-arg GIT_COMMIT=…`, which CI sets); a local build shows none.
