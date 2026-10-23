# Pages

[Back to the README](../README.md)

What each page of the web interface shows and how its numbers come about. The routes behind them are in [API](api.md).

- [Start page](#start-page)
- [Profile page](#profile-page): this week, on this day, charts, listening clock, lost & found, recent listens
- [Year in review](#year-in-review)
- [Artist, album and track pages](#artist-album-and-track-pages)
- [Search](#search)
- [Comparing two profiles](#comparing-two-profiles)
- [Picking a period and a source](#picking-a-period-and-a-source)
- [Other pages](#other-pages)

Profiles of other accounts than the default one have their slug after the user name: `/u/<username>/<slug>/…`. Every page counts in the time zone picked in the settings, or the browser's (see [Settings](accounts.md#settings)), and is shown in English or German.

## Start page

`/` lists the profiles the viewer may see, and those the viewer follows.

## Profile page

`/u/<username>` shows a profile's listens per year, top artists, albums and tracks, the top artists of each year, recent listens, and what is playing now. New listens and "now playing" show up right away while the page is open.

**This week** opens the page when no period is picked: listens against the week before up to the same weekday and time, days with listens, the current and the longest streak of days in a row with listens (left out when the viewer or the profile's owner turned streaks off), listens per day of both weeks and the five artists of the week, marked when heard for the first time. The week runs Monday to Sunday, or from the first weekday picked in the settings. It is left out when the profile had no listens this week or last and no running streak.

**On this day**, below it: today's date in each earlier year with listens on it, the latest first, each with its listens, three most heard artists and five most heard tracks, linking to the profile page for that day. 29 February looks back to the 28th in other years. Left out when there is none.

**Charts:** the top artists, albums and tracks of the period. Each place shows how it moved since the period before: ↑3, ↓2, = or "new". A year is compared with the whole year before, any other period with as many days right before it, all time with nothing. "Show more" below a list adds 20 places at a time. With [album covers](covers.md) switched on, the album chart and the recent listens show them.

**Listening clock:** the listens of the period by weekday and hour of the day, with the busiest day and hour. Hours are local, so 21:00 in summer and 21:00 in winter land in the same cell. The weekdays start with the first day of the week from the settings.

**Lost & found**, further down: the artists and tracks the profile heard a lot (at least 20 listens for an artist, 10 for a track) and not in the year before its latest listen, with their listens and years. The order weighs both: listens times 1 − e^(−years gone / 2), so 39 % a year after the last listen, 63 % after two, 86 % after four; between one and three years gone counts a lot, and ten or fifteen hardly differ. A source narrows only what counts as a lot. The owner can dismiss one they don't want back (an artist takes its tracks along), take it back right away, or show it again later from the list of dismissed ones; dismissals belong to the profile and follow merges.

## Year in review

`/u/<username>/year/2016`, linked from the year view of the profile page: listens against the year before, artists and how many were new, days with listens, the busiest month and day, listens per month of both years, the top artists, albums and tracks, the most heard artists heard for the first time, the biggest risers (artists heard before with the most listens more than in the year before, and their places in both years' charts) and the longest listening session (it goes on while the next listen starts at most 30 minutes after the end of the one before). A running year counts up to now and is compared with the year before up to the same day and time.

## Artist, album and track pages

`/u/<username>/artist/<id>-<name>`, `…/album/…` and `…/track/…`, e.g. `/u/fiona/artist/1-die-arzte`: listens per month, first and last listen, phases of heavy listening, the albums and tracks heard, and a link to MusicBrainz when the entry has an ID. The album page shows the cover when covers are on.

Only the id in the address counts; the name after it is for the reader, and the page moves to the current one when it differs, as after a rename or for links with the id alone. An entry that was merged into another one leads to that one. The months run from the profile's first listen of the entry to its last, leaving out a year or more without any listens (shown as a break).

## Search

`/u/<username>/search?q=`, also from the search field in the header of a profile's pages (`/` jumps into it): the artists, albums and tracks the profile has heard whose names hold every word, ignoring case and accents, the most heard first. A word may also be the artist's name of an album or track ("ärzte unrockbar").

## Comparing two profiles

`/u/<username>/compare/<other>` (with slugs after either name for other profiles), from "Compare" on a profile page: straight against the viewer's own default profile, or else to `/u/<username>/compare` to pick one. It shows two profiles side by side for a period:

- the **overlap**: each profile's listens of an artist as a share of all its listens, the smaller share summed over all artists. 100 % means the same artists equally often, 0 % none in common;
- the artists, albums and tracks **both** heard, ranked by what they add to the overlap;
- the most heard artists of each that the other **never heard at all**.

The viewer must be allowed to see both profiles.

## Picking a period and a source

Under the profile's name the page picks a **period**: the last 7, 30, 90 or 365 days (`?days=30`), a year (`?year=2012`), a span of days (`?from=2009-06-01&to=2009-08-31`, both days included, either may be left out) or all time. It narrows the charts, the listening clock and the listens.

`?source=Navidrome` on any page of a profile (overview, artist, album, track, search) shows only the listens from that **source**. The sources are the clients without their version ("Navidrome 0.64.2 (…)" is "Navidrome"), "Spotify via YourSpotify" and the imports such as `import:php-2016`.

## Other pages

- `/login`.
- `/settings`, see [Settings](accounts.md#settings), with "Edit listening history" per profile, see [Taking out wrong listens](accounts.md#taking-out-wrong-listens).
- `/merges` (admins only), see [Merging in the browser](catalog.md#in-the-browser).
- `/status` (admins only), see [Status page](installation.md#status-page).
