# musicbanana: Entwurf für den nächsten Rewrite

Stand 2026-10-03. Das Schema ist die erste Migration: [backend/migrations/0001_initial.sql](../backend/migrations/0001_initial.sql).

## Was mir an den alten Modellen aufgefallen ist

1. **Artist-Name ist global UNIQUE** (Symfony). Es gibt aber verschiedene Bands mit gleichem Namen (z.B. mehrere "Nirvana"). Das Schema kann sie nicht auseinanderhalten.
2. **Track-Unique `(artist, album, title, tracknumber)`** greift nicht, sobald `album` oder `tracknumber` NULL ist: In PostgreSQL sind NULLs untereinander verschieden, es entstehen also Dubletten. Außerdem wird dasselbe Lied auf Album und Best-of zu zwei Tracks, die Track-Charts zählen es dann doppelt getrennt.
3. **Umbenennen/Zusammenführen** (musicbanana2 `rename()`) schreibt jedes Listen einzeln um und markiert Alte als `obsolete`. Spätere Scrobbles mit der alten Schreibweise legen den alten Artist aber neu an, weil keine Zuordnung "alter Name → neuer Artist" gespeichert wird.
4. **Gut und beibehalten:** Symfony speichert pro Listen die Rohstrings (`artistName`, `albumTitle` …) *und* die Katalog-FKs. Das ist genau richtig, weil man so jederzeit neu zuordnen kann. Dazu Profile pro Konto, `UNIQUE(profile, timestamp)` als Duplikatschutz, und Now-Playing aus musicbanana2.
5. Symfony speichert Zeit doppelt (`date` und `timestamp` int). Eins reicht (`timestamptz`).

## Neues Schema in Kürze

| Bereich | Tabellen | Idee |
|---|---|---|
| Konten | `account`, `profile`, `api_token`, `follow` | Mehrere Profile pro Konto bleiben. Sichtbarkeit public/followers/private. Pro Scrobble-Client ein eigenes Token. Gerichtetes Folgen statt "friends". |
| Katalog | `artist`, `release` (Album), `recording` (Stück) | Nach MusicBrainz-Vorbild. Ein Stück hängt am Artist, nicht am Album. `mbid` optional und eindeutig. Merge über `merged_into` statt `obsolete`. |
| Zuordnung | `artist_alias`, `release_alias`, `recording_alias` | Normalisierter Rohstring → Katalog-Eintrag. Ein Merge biegt den Alias um, damit auch künftige Scrobbles mit alter Schreibweise richtig landen. |
| Scrobbles | `listen`, `now_playing` | Rohdaten unveränderlich, Katalog-FKs änderbar. Index `(profile_id, listened_at DESC)` deckt Charts und "zuletzt gehört" ab. |

Charts (Woche, Monat, Jahr, gesamt; Artist/Album/Track) werden direkt per `GROUP BY` auf `listen` berechnet. Bei einigen hunderttausend Listens pro Profil reicht das mit dem Index. Eine vorberechnete Tagestabelle kommt erst dazu, wenn es messbar langsam wird.
## Entschieden

- **Stack:** Rust-Backend mit **Axum** (HTTP, tower-Middleware) und **sqlx** (rohes SQL, zur Compile-Zeit gegen das Schema geprüft, Offline-Cache in `backend/.sqlx/`). Frontend **SvelteKit + TypeScript + Tailwind** mit `adapter-static` als SPA, die das Backend ausliefert. Node wird nur zum Bauen gebraucht, nicht im Betrieb.
- **Datenbank:** PostgreSQL.
- **Scrobble-API:** ListenBrainz-kompatibel unter `/api/listenbrainz/1/` (`submit-listens`, `validate-token`). Supersonic und Ultrasonic scrobbeln über Navidrome, und Navidrome lässt die ListenBrainz-Adresse umstellen. Pro Client ein Token, gebunden an ein Profil, gespeichert als SHA-256. Prüfungen und Grenzen wie bei listenbrainz-server; Navidrome verwirft einen Listen bei 4xx und versucht es bei 5xx erneut. Neue Namen landen über die Alias-Tabellen im Katalog, fehlende Einträge werden angelegt. Audioscrobbler/last.fm 2.0 nur, falls ein Player es braucht.

## Noch offen

- **Mehrere Profile pro Konto:** Vorschlag beibehalten.
- **MusicBrainz:** Vorschlag zunächst nur `mbid`-Spalten, Abgleich später (Vorarbeit in musicbanana3: `brainz/`, `musicbrainz_notes`).
- **Hosting:** ein Docker-Image (Rust-Binary + statisches Frontend) plus Postgres.

## Altdaten: musicbanana-php (MySQL-Dump von 2016)

5 User, 5.706 Artists, 9.868 Alben, 34.663 Tracks, rund 165.000 Scrobbles (2007-08 bis 2016-05) in je einer Tabelle `mb_usertracks_<user_id>`.

| Alt | Neu |
|---|---|
| `mb_usertracks_N` (`timestamp` Unix-Sekunden, `track_id`, `artist_id`) | `listen` mit `client = 'import:php-2016'`; Rohstrings aus den Katalognamen rekonstruiert |
| `mb_artists` / `mb_albums` / `mb_tracks`, `link_to_*_id` (0 = eigenständig, sonst Merge-Ziel) | `artist` / `release` / `recording` + `merged_into` + Alias-Tabellen; Merge-Ketten werden aufgelöst |
| `mb_tracks.album_id = 0` | kein Album (`release_id` NULL) |
| Track hängt am Album | ein `recording` pro (Artist, normalisierter Titel), Album als `release_id` am Listen |
| `times_played` | entfällt (Charts werden live berechnet) |
| `mb_user.md5_password` (ungesalzen) | `argon2id(md5_hex)`, beim nächsten Login auf echtes argon2id umgestellt |
| `mb_user.friends` (`;2;3`) | `follow` (gerichtet) |
| `mb_session`, `mb_now_playing`, Profilfelder (Obst, Buchstabe, PSYC, Jabber …) | entfällt |

**Kodierung:** Ein Teil der Namen ist doppelt kodiert (UTF-8 als Windows-1252 gelesen und erneut als UTF-8 gespeichert, z.B. `Die Ã„rzte` neben `Die Ärzte`), ein Teil korrekt. Der Import repariert jeden String einzeln (nach cp1252 kodieren, als UTF-8 dekodieren, nur bei gültigem Ergebnis übernehmen). Über die Alias-Tabellen verschmelzen die so entstandenen Dubletten automatisch. Achtung bei Abfragen im Dump: die Collation `_ai_ci` ignoriert Akzente, für Textvergleiche `COLLATE utf8mb3_bin` verwenden.

## Dubletten zusammenführen

`musicbanana merge suggest` schlägt Einträge vor, die wie ein anderer aussehen, jeweils mit dem Befehl zum Zusammenführen (vorerst auf der Kommandozeile, im Browser später mit dem Login):

- **gleiche Buchstaben:** nur Groß-/Kleinschreibung, Akzente, Satzzeichen, Leerzeichen, ein führendes „The“ oder „&“ statt „and“ unterscheiden sich („Bjork“/„Björk“, „AC/DC“/„ACDC“).
- **ein Buchstabe Abstand:** ein Buchstabe mehr, weniger, anders oder mit dem Nachbarn vertauscht („Die Aerzte“/„Die Ärzte“). Erst ab sechs Buchstaben, weil es darunter zu oft ein anderes Wort ist („Blur“/„Blue“), und nie bei verschiedenen Ziffern („Kapitel 1“/„Kapitel 2“).
- **Version:** derselbe Titel ohne Zusatz in Klammern, nach einem Gedankenstrich oder „feat.“ („Unrockbar (Live)“). Das kann eine andere Aufnahme sein, deshalb stehen diese Vorschläge zuletzt.

Der Eintrag mit weniger Listens geht in den mit mehr, eine Version in den Titel ohne Zusatz. Alben und Stücke werden nur innerhalb eines Artists verglichen; ein Artist-Merge nimmt seine Alben und Stücke mit und führt die mit gleichem Titel (in irgendeiner bekannten Schreibweise) mit denen des Ziels zusammen. Ein Merge biegt Listens und Aliase um und setzt `merged_into`, die Rohstrings bleiben unverändert. Rückgängig machen geht noch nicht, daher `--dry-run`.
