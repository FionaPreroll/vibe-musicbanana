# Accounts and profiles

[Back to the README](../README.md)

- [Accounts](#accounts)
- [Profiles and who sees them](#profiles-and-who-sees-them)
- [Following](#following)
- [Settings](#settings)
- [Taking out wrong listens](#taking-out-wrong-listens)
- [Your data](#your-data)
- [Deleting profiles and accounts](#deleting-profiles-and-accounts)
- [Logins](#logins)

Commands below run the binary `musicbanana` (or `cargo run --release --` in `backend/`); with Docker, prefix them with `docker compose exec -it musicbanana`.

## Accounts

New accounts, and new passwords for forgotten ones, come from the command line:

```sh
musicbanana account create <username> --email <address>   # asks for the password, with a default profile
musicbanana account password <username>                     # asks for the new password
musicbanana account rename <username> <new name>
musicbanana account email <username> <address>
musicbanana account admin <username> [--off]               # the status and merges pages
```

Both read the password from standard input when that is not a terminal, e.g. `echo "$PASSWORD" | musicbanana account password fiona`. Passwords need at least 8 characters. The accounts of the old musicbanana log in with their old passwords; the first login replaces the old MD5 hash with a proper one.

**User names** have up to 32 letters, digits, dashes, dots and underscores and start with a letter or digit. A name can be changed on the settings page or with `account rename`. Links with the old name lead to the new one, as long as the viewer may see the profile, and no other account can take the old name; renaming back to it is fine. Scrobble tokens and YourSpotify connections stay as they are.

**The email address** logs in too and can be changed on the settings page (with the password) or with `account email`. musicbanana sends no mail, so nobody checks that it works.

**Admins** see the [status page](installation.md#status-page) and the [merges page](catalog.md#in-the-browser).

## Profiles and who sees them

Every account has a default profile. A further one, for example one per player or for a second person's listens, comes from the settings page or from

```sh
musicbanana profile create --user <username> <slug> --name "Spotify" --visibility private   # public (default), followers or private
```

A profile is **public**, for **followers** (its owner and the accounts it let follow) or **private** (its owner only). To anybody else it does not exist (404), in the list of profiles and on every page and API route below it, album covers included.

## Following

Logged-in accounts can follow a profile with the button next to its name; the start page lists the profiles they follow. A public profile is followed right away. A profile for followers shows strangers a page to ask to follow instead (anonymous visitors still get a 404), and the owner says yes or no under Followers in the settings, where followers can be removed again. Making a profile public answers the open requests with yes. Follows from the old musicbanana ("friends") count as accepted.

## Settings

The page `/settings` has, for the account logged in:

- its profiles (create, rename, who sees them) and their followers;
- scrobble tokens (create, revoke), see [Scrobbling](scrobbling.md#tokens);
- YourSpotify connections (connect, fetch all again, remove), see [Spotify](spotify.md);
- **language**: English or German. Without a choice, the pages follow the browser's language, as they do for visitors who are not logged in. Dates and numbers are written the way the browser's region writes them in the chosen language (Swiss German, say);
- **time zone and first day of the week**: years, weeks and days start at midnight there. Without a time zone the browser's counts; the week starts on Monday unless another day is picked;
- whether to show **streaks** (days in a row with listens) on profile pages, for the viewer's own and everybody else's; a profile's owner turning them off also hides them from visitors;
- user name, email address and password;
- "Your data" (the download below) and a link to "Edit listening history" per profile.

Every page comes light or dark, the dark one in black and banana yellow. It follows the system setting until the moon or sun at the top picks the other one; the browser remembers that pick, and picking what the system shows follows the system again.

## Taking out wrong listens

Wrong listens (a scrobbler that ran twice, a party on your account, a bad import) are taken out on the page "Edit listening history", linked from each profile in the settings. It lists the profile's listens, narrowed by words (artist, track or album, ignoring case and accents), a time span and a source, and moves the ones picked, or all that match, to the trash. Listens in the trash leave the charts and every page, and a player or an import sending them again doesn't bring them back. From the trash they can be restored, with an undo right after moving them, or deleted for good.

## Your data

Under "Your data" in the settings, users download everything musicbanana keeps about their account as one JSON file: the account with its former user names, the profiles with all their listens and their trash, follows both ways, scrobble tokens, YourSpotify connections and current logins. Each listen is in ListenBrainz's import format (`listened_at`, `track_metadata` with the names as the player sent them) plus what musicbanana made of it, so the listens can move to another service. The password hash, the tokens themselves and login cookies stay out. The file is written while it downloads, so big histories need no extra memory.

## Deleting profiles and accounts

Users can't delete profiles or their account themselves; the settings page tells them to ask the admin, who runs

```sh
musicbanana profile delete --user <username> <slug>   # a profile other than the default one, with its listens
musicbanana account delete <username>                 # the account with all its profiles and listens
```

Without `--yes` both only list what would go (profiles, listens, listens in the trash, scrobble tokens, YourSpotify connections, followers) and delete nothing. Artists, albums and tracks stay in the shared catalog. The deleted listens are also taken out of the journal of merges, so `merge undo` can't bring them back, and the user name is free again. There is no undo: back up the database first if in doubt.

## Logins

A login lasts 30 days after the last visit. Changing the password logs out the account's other browsers. After 10 wrong passwords for a name within 15 minutes, that name is refused for the rest of the 15 minutes.

The login cookie is `HttpOnly` and `SameSite=Lax`, and `Secure` when a reverse proxy in front sends `X-Forwarded-Proto: https`, which Caddy, Traefik and nginx (with `proxy_set_header X-Forwarded-Proto $scheme`) do. Requests that change something only take JSON, so other sites can't send them with the cookie.
