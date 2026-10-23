# Album covers

[Back to the README](../README.md)

Albums can show their front cover from the [Cover Art Archive](https://coverartarchive.org/): on the album page, small in the album charts and in the recent listens. Albums without one get a banana-yellow tile with their first letter.

It is **off by default**, since it means requests to the outside. While it is off, pages look as without covers and the server sends nothing anywhere.

| Setting | What |
|---|---|
| `COVER_ART=on` | Fetches covers for albums with a MusicBrainz ID (from Picard-tagged files). |
| `COVER_ART_LOOKUP=on` | Also searches MusicBrainz by artist and title for albums without an ID. Needs `COVER_ART=on`. |
| `COVER_ART_CACHE` | Where covers are kept, default `cache/covers`; in the image `/var/cache/musicbanana/covers`, the compose volume `covers` (see [Optional services](installation.md#optional-services)). |

## How it works

1. The browser asks musicbanana, never the archive: `GET /api/profiles/<username>/<slug>/releases/<id>/cover?size=250`, or `500` for the album page.
2. musicbanana checks that the viewer may see the profile and that the profile has listens of that album. Otherwise the answer is 404, so a private profile's albums stay hidden also as pictures.
3. It takes the album's MusicBrainz release group ID. Without one, and with `COVER_ART_LOOKUP=on`, it asks a search queue (below); a search that has not run yet answers 404 for now, and the cover shows on a later visit.
4. It looks in the cache directory, and on a miss fetches `front-250` or `front-500` of that release group from the archive once and writes it to disk.
5. It sends the image with `Cache-Control: private, max-age=604800`, so the browser keeps it a week.

**Searching by name** (`COVER_ART_LOOKUP`) runs in the background, one search a second as MusicBrainz asks. It takes only a release group with exactly that title and artist (ignoring case), so a cover is missing rather than wrong. What a search found stays in the cache only; it does not become the album's MusicBrainz ID, so a wrong guess can never split or merge the catalog.

**The cache** holds `caa/<mbid>-<size>` (a cover as the archive sent it), `caa/<mbid>-<size>.missing` (the archive has none) and `musicbrainz/<hash>.json` (what a search found). "No cover" and "nothing found" are asked again after 30 days, failed requests (the archive down or slow) on the next view. Each album takes one small file per size viewed. Deleting the directory forgets everything; it fills up again as pages are viewed, and it needs no backup.

## What leaves the server, and what guards it

- **Nothing unless switched on.** Both settings are off by default; an unknown value such as `COVER_ART=yes` stops the server at start instead of guessing.
- **The browser talks only to musicbanana.** The archive and MusicBrainz never see a visitor's address, cookies or which pages they view; the server's requests carry its own user agent and nothing about the viewer.
- **Fixed hosts.** The server only starts requests at `https://coverartarchive.org/` and `https://musicbrainz.org/`; the archive answers with a redirect to the file at archive.org, which the server follows. The address is built from a MusicBrainz ID, which is a UUID by type, or for searches from artist and title as query parameters (quoted for MusicBrainz's search syntax). No user input can pick the host or the path, so the cover route cannot be used to make the server fetch from elsewhere in your network.
- **Only the albums of a visible profile.** The same visibility check as every other profile route, plus "the profile has listens of it". Guessing album ids of a private profile gives 404 just like a missing album.
- **Only two sizes.** `size` is 250 or 500, anything else is a 400, so nobody can fill the disk with other variants. Files on disk are named from the UUID and the size, never from names sent by a client.
- **Limits on what comes back.** Each request gives up after 15 seconds. A cover larger than 5 MB is refused, by its announced length before reading and by its real length after. Only answers that start like a JPEG, PNG, GIF or WebP image are kept and served (checked by their first bytes, not by what the archive says), with that media type and `X-Content-Type-Options: nosniff`, so a browser never treats a cover as HTML or script. The same check runs again when a file is read back from the cache.
- **No half files.** A cover is written to a file of its own and then renamed into place, so a reader never sees half a cover, also when two requests fetch the same one.
- **Gentle with MusicBrainz.** Searches go through one queue at one per second, and results, also "nothing found", are kept, so a page with many albums does not cause a burst of requests.

The code is in `backend/src/covers.rs`; its tests use stand-ins for the archive and MusicBrainz.
