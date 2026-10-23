# Spotify via YourSpotify

[Back to the README](../README.md)

[YourSpotify](https://github.com/Yooooomi/your_spotify) keeps the history of a Spotify account: the plays from Spotify's data export and those it has fetched from Spotify since. musicbanana takes them from there into a profile, either through a connection the server keeps up to date (the simple way) or with the importer on the command line.

A play counts for its first artist, with track and album titled as on Spotify; the Spotify IDs and all artists stay in the listen's extra data. Spotify gives no MusicBrainz IDs. Its titles often carry an edition like "(Deluxe Edition)" or "- Remastered 2011", which the catalog leaves out (see [Remasters and deluxe editions](catalog.md#remasters-and-deluxe-editions); plays imported before that need `merge editions` once). Other additions such as "- Live" stay, and `merge suggest` finds those next to the albums and tracks from other players.

## Connections the server keeps up to date

A profile can be connected to a YourSpotify on the settings page under "Spotify via YourSpotify" (profile, API address, public token), or with

```sh
export YOURSPOTIFY_TOKEN=…
musicbanana yourspotify add --from http://192.168.1.10:8080 --user <username> --profile <slug>
musicbanana yourspotify list                 # all connections with their state
musicbanana yourspotify refetch <id>         # fetch the whole history again
musicbanana yourspotify remove <id>
```

The token is tried before the connection is kept. From then on `musicbanana serve` imports the whole history once and afterwards the new plays every 15 minutes, as the importer below would; the settings page shows when it last finished, how many plays it brought in so far and the last error. Each profile takes one connection, so two Spotify accounts go to two profiles, of the same account or of different ones.

**The token** is stored in the database and never shown again, logged or sent to the browser; removing the connection deletes it.

**Allowed addresses:** the settings page only takes the addresses in `YOURSPOTIFY_ALLOWED_URLS` (separated by commas, e.g. `http://192.168.1.10:8080`; an address also allows the paths below it, with the same scheme, host and port), so that nobody can make the server fetch from elsewhere in your network; without it the page takes none, and the command line takes any address. The server does not follow redirects from YourSpotify. Connections made before an address left the list keep running; `yourspotify list` shows them and `yourspotify remove` stops them.

**Fetch all again:** when YourSpotify has imported an older Spotify export after the connection was made, its plays are older than the latest one imported, so the rounds every 15 minutes miss them. **Fetch all again** on the settings page (or `yourspotify refetch <id>`) has the server fetch the whole history once more, within a minute or once the import that runs is done, oldest first as the first time; plays the profile has already are skipped, as with scrobbles. The settings page shows how far it has got, and when it is done how many plays it found, how many were new and how many were there already. One that stops (YourSpotify down, the server restarted) goes on from where it stopped in the next round; afterwards the connection fetches the new plays every 15 minutes as before.

## The importer

```sh
export YOURSPOTIFY_TOKEN=…    # the public token from YourSpotify's settings
musicbanana import-yourspotify --from http://192.168.1.10:8080 --user <username>
```

`--from` is the address of YourSpotify's API (what YourSpotify has as `API_ENDPOINT`), not of its web interface. The public token gives read access to all statistics of the account, so keep it like a password; set as `YOURSPOTIFY_TOKEN` it stays out of the shell history, `--token` works too.

The first run fetches the whole history, oldest first, a month at a time, and stores each month before fetching the next, so the plays show up in the profile while it runs; a run that stops halfway goes on from there the next time. Later runs fetch only the plays after the latest one imported. `--all` fetches everything again, for example after YourSpotify has imported an older Spotify export; plays the profile has already are skipped, as with scrobbles. The plays go to the default profile, `--profile <slug>` picks another one, and `--every 15m` keeps the command running and fetches the new plays every 15 minutes (or `90s`, `1h`). With Docker, the compose service `yourspotify` runs it this way, see [Optional services](installation.md#optional-services).

## YourSpotify's API

YourSpotify has no documented API; musicbanana uses the route its web interface reads the history from (`GET /spotify/gethistory`), which a new YourSpotify version could change. A YourSpotify that takes no time range there gets the old way: everything fetched newest first, then stored at once.
