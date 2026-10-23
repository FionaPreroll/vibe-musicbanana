# Catalog

[Back to the README](../README.md)

All profiles share one catalog of artists, albums and tracks; a listen keeps the names as they were sent and points at an entry of the catalog. These tools keep it tidy. Commands run the binary `musicbanana`; with Docker, prefix them with `docker compose exec musicbanana`.

- [Merging duplicates](#merging-duplicates) on the command line and [in the browser](#in-the-browser)
- [Remasters and deluxe editions](#remasters-and-deluxe-editions)
- [Renaming and MusicBrainz IDs by hand](#renaming-and-musicbrainz-ids-by-hand)

## Merging duplicates

The same artist, album or track can end up in the catalog under several spellings: "Die Aerzte" next to "Die Ärzte", "Bjork" next to "Björk", typos, or "Unrockbar (Live)" next to "Unrockbar". `merge suggest` lists look-alikes, each line with the command that merges them:

```sh
musicbanana merge suggest                     # artists, releases and recordings, 30 each
musicbanana merge suggest artists --limit 100
musicbanana merge artist 977 30 --dry-run     # what merging artist 977 into 30 would change
musicbanana merge artist 977 30
```

A suggestion says why:

- **same letters:** only case, accents, punctuation, spaces, a leading "The" or "&" for "and" differ;
- **one letter apart:** a letter more, missing, different or swapped with its neighbour, in names of six letters or more and never where digits differ ("Chapter 1", "Chapter 2");
- **version:** the same title apart from a note in brackets, after a dash or "feat.", so possibly a live version; these come last.

The entry with fewer listens goes into the one with more, a version into the plain title. Releases and recordings are compared within one artist only, so merge artists first: merging an artist also merges its releases and recordings into the other artist's ones with the same title and moves the rest over. A merge points the listens and the spellings at the remaining entry, so later scrobbles with the old spelling land there too; the raw strings of the listens stay as they were.

**MusicBrainz IDs:** two entries that both have an ID are not suggested, as their IDs say they are different ones of the same name, and merging them takes `--force`: for an artist's other name that should count for the main one, or a recording MusicBrainz lists twice. Merging an artist moves its albums and tracks over instead of merging them with one of another ID. A merge takes the IDs along, so listens with the ID of the merged entry count for the remaining one.

### Undo

Every merge can be taken back. A merge prints its number, and `merge log` lists them:

```sh
musicbanana merge log                  # the latest 30, newest first
musicbanana merge undo 12 --dry-run    # what undoing merge 12 would put back
musicbanana merge undo 12
```

Undoing puts back what the merge changed: the listens, spellings and MusicBrainz IDs go back to the merged entry, which stands on its own again, and an artist gets its albums and tracks back. What changed since stays as it is now, such as listens that came in under the merged spelling after the merge, and the command says how many such rows it left. A merge that a later one built on (merging the remaining entry on into a third one, say) can only be undone after that later one.

### In the browser

Admins (see [Accounts](accounts.md#accounts)) have a Merges link at the top that leads to `/merges`, which does the same as the commands:

- the suggestions of artists, albums and tracks, each with a preview of what merging would change (the dry run) before it merges, the other way round, or hidden for good;
- a merge by hand of any two entries, found by name or `#id`;
- the log of all merges, also those from the command line, each with a preview before it is undone.

A hidden suggestion (a live version that should stay apart, say) leaves `merge suggest` too; the page lists the hidden ones and can suggest them again. Merges and undos from the page go into the log under `musicbanana::merge`, with the account that made them.

## Remasters and deluxe editions

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

The number is the entry's id, as in the address of its page (`/u/<name>/artist/12`) or shown by `merge suggest`.

A rename keeps the old spellings leading to the entry, so later listens under the old name still count for it, and the new name becomes a spelling too unless it leads to another entry already. Renaming one of two artists of the same name, say to "Nirvana (UK)", tells them apart in the charts; its MusicBrainz ID keeps counting under the old name.

`mbid add` gives an entry a MusicBrainz ID, given as such or as the address of its MusicBrainz page; an album (release) takes the ID of its release group. Listens with that ID count for the entry from then on, while the listens so far stay where they are. `mbid remove` takes an ID away again, such as the ID of a live version that came first and went to the track with all the old listens: the next listen with the ID is then matched like one with a new ID, and `merge` can join the studio version into the old track.
