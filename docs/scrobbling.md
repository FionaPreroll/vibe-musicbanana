# Scrobbling

[Back to the README](../README.md)

musicbanana speaks the part of the [ListenBrainz API](https://listenbrainz.readthedocs.io/en/latest/users/api/core.html) that players use to scrobble, under `/api/listenbrainz/1/`: `POST submit-listens` (`single`, `import` and `playing_now`) and `GET validate-token`. Limits, checks and error responses follow listenbrainz-server.

A listen keeps the strings and extra data (`additional_info`) exactly as sent and is matched to the catalog through the alias tables; unknown artists, albums and tracks are created.

**Duplicates are skipped**, so clients can safely resend:

- a second listen at the same second;
- a listen of the same track that follows another one sooner than a player counts a play (half the track, at most four minutes, 15 seconds when the length is unknown): nobody plays a track twice that fast, so it comes from a second scrobbler or a play sent twice.

## Tokens

Every client gets its own token, which belongs to one profile. Tokens are created on the settings page, or on the command line:

```sh
musicbanana token create --user <username> --label Navidrome                    # prints the token, only this once
musicbanana token create --user <username> --profile <slug> --label "Work laptop"
musicbanana token list [--user <username>]
musicbanana token revoke <id>
```

## Navidrome, and apps that play from it (Supersonic, Ultrasonic, …)

Subsonic apps report plays to Navidrome, and Navidrome passes them on to ListenBrainz, so pointing Navidrome at musicbanana covers all of them:

1. Set Navidrome's ListenBrainz address to musicbanana and restart Navidrome, in `navidrome.toml`:
   ```toml
   ListenBrainz.BaseURL = "https://musicbanana.example.org/api/listenbrainz/1/"
   ```
   or as the environment variable `ND_LISTENBRAINZ_BASEURL` (with Docker see [Connecting Navidrome](installation.md#connecting-navidrome)). This holds for every user of that Navidrome: they can scrobble to musicbanana (each with their own token), but no longer to listenbrainz.org. Last.fm scrobbling keeps working.
2. In Navidrome's personal settings, turn on "Scrobble to ListenBrainz" and paste the token. Navidrome checks it right away.
3. Leave scrobbling switched on in the apps. "Now playing" shows on the profile page while a track plays.

Tested with Navidrome 0.64.2: linking the token, "now playing" and listens sent through its Subsonic `scrobble` endpoint.

## Other ListenBrainz clients

Clients that let you change the ListenBrainz server want either the API root `https://musicbanana.example.org/api/listenbrainz/1/` or the server `https://musicbanana.example.org/api/listenbrainz`, plus the token.

## Artists, albums and tracks of the same name

Listens of files tagged with MusicBrainz Picard come with MusicBrainz IDs, which Navidrome passes on. musicbanana uses those of the artist (for a track by one artist), of the album's release group and of the recording:

- Artists, albums or tracks of the same name with different IDs stay apart, such as two bands called Nirvana, two albums called "Weezer", or a song and its live version. The first ID that comes with a name goes to the entry listened to so far, imported listens included, and listens without an ID keep counting for that one.
- A track ID is that of the MusicBrainz recording, the same audio on whatever album or single. A remaster mostly has the ID of the original and counts for the same track; a live version or a new recording has one of its own.
- An album ID finds its album under any title, so "Geräusch (Deluxe)" counts for "Geräusch" when both belong to the same release group, and that title does from then on without an ID too. Likewise for a recording ID and its track.
- An artist ID that came with another name before is not used: a client may send the ID of A along with "A & B", and then the listens of A must not count for "A & B".

The pages of an artist, album or track link to MusicBrainz. IDs can also be added and removed by hand, see [Catalog](catalog.md#renaming-and-musicbrainz-ids-by-hand).
