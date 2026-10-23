# Album covers

[Back to the README](../README.md)

Albums can show their front cover from the [Cover Art Archive](https://coverartarchive.org/): on the album page, small in the album charts and in the recent listens. Albums without one get a banana-yellow tile with their first letter.

It is **off by default**, since it means requests to the outside. While it is off, pages look as without covers and the server sends nothing anywhere.

| Setting | What |
|---|---|
| `COVER_ART=on` | Fetches covers for albums with a MusicBrainz ID (from Picard-tagged files). |
| `COVER_ART_LOOKUP=on` | Also searches MusicBrainz by artist and title for albums without an ID. Needs `COVER_ART=on`. |
| `COVER_ART_CACHE` | Where covers are kept, default `cache/covers`; in the image `/var/cache/musicbanana/covers`, the compose volume `covers` (see [Optional services](installation.md#optional-services)). |
| `COVER_ART_CACHE_MAX` | The most the cache may hold, default `1G`, at least `10M`; `500M`, `2G` and plain bytes work, K/M/G counting in steps of 1024. |

## How it works

1. The browser asks musicbanana, never the archive: `GET /api/profiles/<username>/<slug>/releases/<id>/cover?size=250`, or `500` for the album page.
2. musicbanana checks that the viewer may see the profile and that the profile has listens of that album. Otherwise the answer is 404, so a private profile's albums stay hidden also as pictures.
3. It takes the album's MusicBrainz release group ID. Without one, and with `COVER_ART_LOOKUP=on`, it asks a search queue (below); a search that has not run yet answers 404 for now, and the cover shows on a later visit.
4. It looks in the cache directory, and on a miss fetches `front-250` or `front-500` of that release group from the archive once and writes it to disk.
5. It sends the image with `Cache-Control: private, max-age=604800`, so the browser keeps it a week.

**Searching by name** (`COVER_ART_LOOKUP`) runs in the background, one search a second as MusicBrainz asks. It takes only a release group with exactly that title and artist (ignoring case), so a cover is missing rather than wrong. What a search found stays in the cache only; it does not become the album's MusicBrainz ID, so a wrong guess can never split or merge the catalog.

**The cache** holds `caa/<mbid>-<size>` (a cover as the archive sent it), `caa/<mbid>-<size>.missing` (the archive has none) and `musicbrainz/<hash>.json` (what a search found). "No cover" and "nothing found" are asked again after 30 days, failed requests (the archive down or slow) on the next view. Each album takes one small file per size viewed. Past `COVER_ART_CACHE_MAX`, the covers seen longest ago are deleted until the cache is down to 90 % of it; they come back when viewed again. Deleting the directory forgets everything; it fills up again as pages are viewed, and it needs no backup.

## What leaves the server, and what guards it

- **Nothing unless switched on.** Both settings are off by default; an unknown value such as `COVER_ART=yes` stops the server at start instead of guessing.
- **The browser talks only to musicbanana.** The archive and MusicBrainz never see a visitor's address, cookies or which pages they view; the server's requests carry its own user agent and nothing about the viewer.
- **Only the albums of a visible profile.** The same visibility check as every other profile route, plus "the profile has listens of it". Guessing album ids of a private profile gives 404 just like a missing album.
- **Nothing a visitor sends goes into an address.** Addresses are built from the fixed hosts `coverartarchive.org` and `musicbrainz.org`, an ID out of the database and the size 250 or 500 (anything else is refused with 400). Cache file names are only such IDs or a hash, so no name can point outside the cache directory.
- **Redirects only to archive.org.** The archive sends covers on to archive.org. Redirects are followed at most 5 times and only back to the archive or over HTTPS, on the standard port, to `archive.org` and its subdomains; never to an IP address, another port or an address with a user name. Any other redirect is refused, the cover counts as missing for 30 days, and the server logs a warning (target `musicbanana::covers`) with both addresses, shortened to scheme, host, port and path (a query could hold a secret), with everything but printable ASCII escaped and cut to 200 characters. Searches at MusicBrainz follow no redirect at all.
- **Bounded answers.** Answers are read in pieces and dropped as soon as they grow past 5 MB for a cover or 1 MB for a search, also when they gave no length or a wrong one. Only JPEG, PNG, GIF and WebP are served, recognized by their first bytes, never SVG or HTML, with the server's own `Content-Type` and `X-Content-Type-Options: nosniff`, so a browser never treats a cover as HTML or script. The same check runs again when a file is read back from the cache.
- **Bounded work.** At most 4 covers are fetched at once, each within 15 seconds; at most 1000 searches wait, more are asked again on a later view. Searches go one a second, and their results, also "nothing found", are kept, so a page with many albums causes no burst of requests. The cache stays below `COVER_ART_CACHE_MAX`.
- **No half files.** A cover is written to a file of its own and then renamed into place, so a reader never sees half a cover, also when two requests fetch the same one.

## Tests

The code is in `backend/src/covers.rs`. `backend/tests/covers.rs` and `backend/tests/cover_hardening.rs` run against stand-ins for both services (redirects elsewhere and in a loop, oversized answers with and without a length, a full queue, the cache limit), and property tests in `backend/src/covers.rs` ([proptest](https://proptest-rs.github.io/proptest/)) try random addresses, bytes and names on the checks above.
