# API

[Back to the README](../README.md)

Everything the web interface shows comes from JSON routes under `/api`. Which page uses which route is in [Pages](pages.md); the scrobble routes for players are in [Scrobbling](scrobbling.md).

## Rules for all routes

- **Visibility:** a profile the viewer may not see does not exist (404), on every route below `/api/profiles/<username>/<slug>` and in the list of profiles; see [Profiles and who sees them](accounts.md#profiles-and-who-sees-them).
- **Logging in:** routes under `/api/me` need a login (the cookie from `POST /api/session`), routes under `/api/admin` an admin. Requests that change something only take JSON.
- **Time:** years and days start at midnight in `tz`, an IANA name such as `Europe/Berlin`, UTC by default. The frontend sends the time zone from the settings, or the browser's.
- **Periods:** `from` and `to` are the first and last day of a period, both included, and either can be left out; `year=2012` is short for the whole year.
- **Sources:** every route of a profile takes `source=` (e.g. `Navidrome`, `Spotify via YourSpotify`, `import:php-2016`) and then counts only that source's listens.
- **Paging:** `listens` and the history page backwards: pass a page's `next` as `before`. Charts page with `limit` and `offset`.
- **Ids:** only the id in the address of an artist, album or track counts; an entry that was merged into another one answers with that one.

## Profiles

All below `/api/profiles/<username>/<slug>`, except the first.

| Route | What |
|---|---|
| `GET /api/profiles` | The profiles the viewer may see. |
| `GET …?tz=` | Overview: listens per year, first and last listen. |
| `GET …/top/{artists,releases,recordings}?year=&from=&to=&tz=&limit=&offset=&compare=` | A chart. Each entry has its `rank`; with `compare=true` also `previous_rank` in the period before, `null` for new. |
| `GET …/top/artists/years?tz=&limit=` | The top artists of each year. |
| `GET …/listens?before=&limit=` | Recent listens, newest first. |
| `GET …/now-playing` | What plays now, `null` when nothing. |
| `GET …/live` | Server-Sent Events `now-playing` and `listens` whenever either changed, also from other processes such as `import-yourspotify --every` (PostgreSQL `NOTIFY`). A reverse proxy must not buffer it; nginx leaves it alone because of the `X-Accel-Buffering: no` header. |
| `GET …/sources` | The sources with their listens. |
| `GET …/week?day=&tz=&week_start=` | This week. `day` picks another week (today by default); `week_start` is 1 for Monday, the default, to 7 for Sunday. |
| `GET …/on-this-day?day=&tz=` | On this day; `day` looks back from another date. |
| `GET …/clock?year=&from=&to=&tz=` | The listening clock: `listens` and `weekdays`, Monday to Sunday with the listens of the hours 0 to 23. |
| `GET …/lost-and-found?limit=` | Lost & found. |
| `GET …/year/<year>?tz=` | The year in review. |
| `GET …/{artists,releases,recordings}/<id>?tz=` | An artist, album or track. |
| `GET …/releases/<id>/cover?size=250\|500` | An album cover, see [Album covers](covers.md). 404 when covers are off or there is none. |
| `GET …/search?q=&limit=` | Search. |
| `GET`, `PUT`, `DELETE …/follow` | Whether the viewer follows the profile; follow (or ask to); unfollow. |
| `GET /api/compare/<username>/<slug>/<other>/<other slug>?year=\|from=&to=&tz=&limit=` | Two profiles compared, see [Comparing two profiles](pages.md#comparing-two-profiles). |
| `GET /api/renamed/<old name>/<slug>` | The current name after a rename. |

## The account logged in

| Route | What |
|---|---|
| `POST /api/session` (`{"login", "password"}`), `DELETE /api/session` | Log in (user name or email address), log out. |
| `GET /api/me` | The account and its profiles, without their listens. |
| `GET`, `POST /api/me/profiles`, `PATCH /api/me/profiles/<slug>` | The profiles with their listens; create one (`{"slug", "name", "visibility"}`); rename one or change who sees it. |
| `GET /api/me/following`, `GET /api/me/followers` | Profiles followed; followers and open requests. |
| `PUT`, `DELETE /api/me/followers/<slug>/<username>` | Accept a request, remove a follower. |
| `GET`, `POST /api/me/tokens` (`{"profile", "label"}`), `DELETE /api/me/tokens/<id>` | Scrobble tokens; a new token is shown once. |
| `GET`, `POST /api/me/yourspotify` (`{"profile", "url", "token"}`), `DELETE /api/me/yourspotify/<id>` | YourSpotify connections. |
| `POST /api/me/yourspotify/<id>/refetch`, `GET /api/me/yourspotify/allowed` | Fetch all again; the addresses from `YOURSPOTIFY_ALLOWED_URLS`. |
| `PUT /api/me/time` (`{"time_zone", "week_start"}`) | `time_zone` null for the browser's, `week_start` 1 for Monday to 7 for Sunday. |
| `PUT /api/me/streaks` (`{"show_streaks"}`) | Show streaks or not. |
| `PUT /api/me/language` (`{"language"}`) | A language code such as `de`, null for the browser's. |
| `PUT /api/me/username` (`{"username"}`), `PUT /api/me/email` (`{"email", "password"}`), `PUT /api/me/password` (`{"current", "new"}`) | User name, email address, password. |
| `GET /api/me/profiles/<slug>/history?q=&from=&to=&source=&before=&limit=` | "Edit listening history": the profile's listens, filtered. |
| `GET`, `POST /api/me/profiles/<slug>/trash` | The trash; move listens there (`{"ids"}`, or the filter of the history to move all that match). |
| `POST …/trash/restore`, `POST …/trash/empty` (`{"ids"}`, or nothing for all) | Restore listens, delete them for good. |
| `GET`, `POST /api/me/profiles/<slug>/lost-and-found/hidden` (`{"kind": "artist"\|"recording", "id"}`), `DELETE …/hidden/<kind>/<id>` | Dismissed entries of Lost & found. |
| `GET /api/me/export` | Everything about the account as one JSON file, see [Your data](accounts.md#your-data). |

## Admins

| Route | What |
|---|---|
| `GET /api/admin/status` | The status page. |
| `GET /api/admin/merges/suggestions/<artist\|release\|recording>?limit=` | Merge suggestions. |
| `GET`, `POST /api/admin/merges/hidden/<kind>` (`{"from", "into"}`), `DELETE /api/admin/merges/hidden/<kind>/<from>/<into>` | Hidden suggestions. |
| `GET /api/admin/catalog/<kind>?q=` | Find entries by name or `#id`, for merging by hand. |
| `POST /api/admin/merges` (`{"kind", "from", "into", "dry_run", "force"}`) | Merge, or preview with `dry_run`. |
| `GET /api/admin/merges?before=&limit=`, `POST /api/admin/merges/<n>/undo` (`{"dry_run"}`) | The merge log; undo a merge. |

## Other

| Route | What |
|---|---|
| `GET /api/config` | What the server offers the frontend: `{"covers"}`. |
| `GET /api/health` | For health checks. |
| `POST /api/listenbrainz/1/submit-listens`, `GET /api/listenbrainz/1/validate-token` | Scrobbling, see [Scrobbling](scrobbling.md). |
