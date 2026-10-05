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
| Konten | `account`, `profile`, `api_token`, `follow`, `session`, `former_username` | Mehrere Profile pro Konto bleiben. Sichtbarkeit public/followers/private. Pro Scrobble-Client ein eigenes Token. Gerichtetes Folgen statt "friends"; bei Profilen nur für Follower ist es eine Anfrage (`accepted_at` leer), bis der Besitzer zustimmt. Browser-Logins als `session` (SHA-256 des Cookie-Tokens, 30 Tage ab dem letzten Besuch). Alte Benutzernamen nach dem Umbenennen in `former_username`: Links damit führen zum neuen Namen, kein anderes Konto kann sie nehmen. |
| Katalog | `artist`, `release` (Album), `recording` (Stück) | Nach MusicBrainz-Vorbild. Ein Stück hängt am Artist, nicht am Album. Merge über `merged_into` statt `obsolete`. |
| Zuordnung | `artist_alias`, `release_alias`, `recording_alias`; `artist_mbid`, `release_mbid`, `recording_mbid` | Normalisierter Rohstring → Katalog-Eintrag, ebenso MusicBrainz-ID → Eintrag. Ein Merge biegt beides um, damit auch künftige Scrobbles mit alter Schreibweise oder ID richtig landen. |
| Merge-Protokoll | `merge_op`, `merge_change` | Jeder Merge schreibt per Trigger jede geänderte Zeile (Schlüssel, alte und neue Werte) mit; `undo_merge()` spielt sie rückwärts zurück, nur wo die Zeile seitdem unverändert ist. |
| Scrobbles | `listen`, `now_playing`, `listen_trash` | Rohdaten unveränderlich, Katalog-FKs änderbar. Index `(profile_id, listened_at DESC)` deckt Charts und "zuletzt gehört" ab. Vom Besitzer gelöschte Listens liegen mit ihrer id in `listen_trash`, bis er sie zurückholt oder endgültig löscht; alle anderen Abfragen lesen nur `listen`. |

Charts (Woche, Monat, Jahr, gesamt; Artist/Album/Track) werden direkt per `GROUP BY` auf `listen` berechnet. Bei einigen hunderttausend Listens pro Profil reicht das mit dem Index. Eine vorberechnete Tagestabelle kommt erst dazu, wenn es messbar langsam wird.
## Entschieden

- **Stack:** Rust-Backend mit **Axum** (HTTP, tower-Middleware) und **sqlx** (rohes SQL, zur Compile-Zeit gegen das Schema geprüft, Offline-Cache in `backend/.sqlx/`). Frontend **SvelteKit + TypeScript + Tailwind** mit `adapter-static` als SPA, die das Backend ausliefert. Node wird nur zum Bauen gebraucht, nicht im Betrieb.
- **Betrieb:** ein Docker-Image (Rust-Binary + statisches Frontend, `Dockerfile`), das die CI für jeden Stand von main als `ghcr.io/fionapreroll/vibe-musicbanana` veröffentlicht, mit eigener Postgres in `deploy/compose.yaml`, auf dem Rechner neben Navidrome, das ebenfalls in Docker läuft. Navidrome erreicht die Scrobble-API über den veröffentlichten Port oder über ein gemeinsames Docker-Netz.
- **Datenbank:** PostgreSQL.
- **Scrobble-API:** ListenBrainz-kompatibel unter `/api/listenbrainz/1/` (`submit-listens`, `validate-token`). Supersonic und Ultrasonic scrobbeln über Navidrome, und Navidrome lässt die ListenBrainz-Adresse umstellen. Pro Client ein Token, gebunden an ein Profil, gespeichert als SHA-256. Prüfungen und Grenzen wie bei listenbrainz-server; Navidrome verwirft einen Listen bei 4xx und versucht es bei 5xx erneut. Neue Namen landen über die Alias-Tabellen im Katalog, fehlende Einträge werden angelegt. Duplikate: dieselbe Sekunde im selben Profil, oder dasselbe Stück näher an einem vorhandenen Listen, als ein Player zum Zählen braucht (halbe Länge, höchstens vier Minuten, ohne bekannte Länge 15 Sekunden). So schnell kann niemand ein Stück zweimal hören. Audioscrobbler/last.fm 2.0 nur, falls ein Player es braucht.

## Noch offen

- **Mehrere Profile pro Konto:** Vorschlag beibehalten.
- **MusicBrainz:** Abgleich mit der MusicBrainz-Datenbank (Namen, Erläuterungen wie „US grunge band“) später (Vorarbeit in musicbanana3: `brainz/`, `musicbrainz_notes`).

## Gleiche Namen, verschiedene Einträge

Scrobbles von Dateien, die mit MusicBrainz Picard getaggt sind, bringen MusicBrainz-IDs mit (bei Navidrome `artist_mbids`, `release_group_mbid`, `recording_mbid`). Sie trennen, was gleich heißt:

- **Artists:** Gleichnamige Artists mit verschiedener ID bleiben getrennt (zwei Bands namens Nirvana). Die erste ID zu einem Namen bekommt der Eintrag, der bisher so hieß, samt importierter Listens; Listens ohne ID zählen weiter für ihn. Der Eintrag einer zweiten ID bekommt keinen Alias. Eine ID zählt nur bei einem Stück mit einem einzigen Artist, und nur unter einem Namen, unter dem sie schon vorkam: Clients schicken bei „A & B“ manchmal die ID von A mit.
- **Alben:** wie Artists, mit der ID der Release Group (alle Ausgaben eines Albums), pro Artist, weil ein Sampler hier ein Album jedes Artists darauf ist. Zwei Alben namens „Weezer“ bleiben getrennt, ebenso eine Single, die wie ihr Album heißt. Eine bekannte ID findet ihr Album unter jedem Titel („Geräusch (Deluxe)“), der Titel wird dabei zur weiteren Schreibweise.
- **Stücke:** wie Alben, mit der Recording-ID. Eine Recording ist in MusicBrainz dieselbe Aufnahme, egal auf welchem Album oder welcher Single: Ein Remaster hat meist die ID des Originals und zählt mit, eine Live-Version oder Neuaufnahme hat eine eigene und wird ein eigenes Stück gleichen Titels. Listens ohne ID zählen für das erste; was im Import schon beisammen ist (Studio und live vor 2016), bleibt es. Eine bekannte ID findet ihr Stück unter jedem Titel.
- **Merge:** Einträge, die beide eine ID haben, werden nicht vorgeschlagen und nur mit `--force` zusammengeführt, etwa ein anderer Name eines Artists, der für den Hauptnamen zählen soll, oder eine Recording, die MusicBrainz doppelt führt. Ein Artist-Merge führt Alben und Stücke mit verschiedenen IDs nicht zusammen. Ein Merge nimmt die IDs mit.
- **Nebenläufigkeit:** Kommen zwei neue IDs für denselben Namen gleichzeitig, sperrt die Zuordnung den bisherigen Eintrag, damit ihn nur eine übernimmt.
- **Von Hand:** `musicbanana rename` benennt einen Eintrag um; die alten Schreibweisen führen weiter zu ihm, der neue Name wird eine weitere, sofern er nicht schon zu einem anderen Eintrag führt. So lassen sich zwei gleichnamige Artists in den Charts unterscheiden. Damit die ID des zweiten Nirvana (ohne eigene Schreibweise) unter „Nirvana“ weiter zählt, merkt sich `artist_mbid.name_key` den Namen, den der Artist hatte, als die ID zu ihm kam. `musicbanana mbid add|remove` gibt einem Eintrag eine ID oder nimmt sie ihm, etwa wenn zuerst die Live-Version kam und die alte Historie ihre ID bekommen hat; bisherige Listens bleiben, wo sie sind.

## Seiten pro Artist, Album und Stück

Jedes Profil hat eine Seite pro Artist, Album und Stück (`/u/<name>/artist/<id>`, `…/album/<id>`, `…/track/<id>`): Zahl der Listens, erster und letzter Listen, die Listens pro Monat als Kurve und darunter die meistgehörten Alben und Stücke. Die Adresse eines zusammengeführten Eintrags führt zu dem, in den er gegangen ist.

- **Monate:** vom ersten bis zum letzten Listen des Profils, damit alle Seiten eines Profils dieselbe Zeitachse haben. Ein Jahr oder mehr ganz ohne Listens im Profil, etwa zwischen dem Import von 2016 und neuen Scrobbles, fällt heraus und erscheint in der Kurve als schmaler Bruch.
- **Phasen intensiven Hörens:** Monate mit mindestens doppelt so vielen Listens wie im Schnitt und mindestens drei, zusammengefasst über einzelne ruhigere Monate dazwischen. Der Schnitt zählt nur die Monate vom ersten bis zum letzten, in dem der Eintrag gehört wurde, sonst wäre alles eine Phase, was erst spät dazukam. Eine Phase braucht mindestens 3 % aller Listens des Eintrags; gezeigt werden höchstens die fünf größten.

## Lieblinge pro Zeitraum

- **Top-Listen für einen Zeitraum:** Die Top-Artists, -Alben und -Stücke des Profils gibt es für alle Zeit, die letzten 7, 30, 90 oder 365 Tage (heute mitgezählt), ein Kalenderjahr oder beliebige Tage von–bis, auch nach einer Seite offen (`?days=30`, `?year=2012`, `?from=2009-06-01&to=2009-08-31`). Tage und Jahre beginnen um Mitternacht in der Zeitzone des Browsers. Die zuletzt gehörten Listens fangen am Ende des Zeitraums an.
- **Top-Artists über die Jahre:** ein Rangdiagramm mit einer Spalte pro Jahr, darin die zehn meistgehörten Artists in der Reihenfolge der Jahres-Top-Liste (bei Gleichstand alphabetisch). Eine Linie verbindet die Plätze eines Artists in zwei aufeinanderfolgenden Jahren. Die sechs Artists, die am längsten dabei sind, bekommen eine Farbe, alle anderen bleiben grau; Jahre ohne Listens erscheinen als schmaler Bruch. Das Diagramm hängt nicht vom gewählten Zeitraum ab, hebt aber ein gewähltes Jahr hervor.

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

## Spotify über YourSpotify

- **Quelle:** YourSpotify statt der Spotify-API direkt: Es hat schon den Export-Dump und holt laufend neue Plays, musicbanana braucht also keine eigene Spotify-App und keinen Spotify-Login. YourSpotify hat keine dokumentierte API; der Import nimmt die Route, mit der seine Weboberfläche die Historie zeigt (`GET /spotify/gethistory`, höchstens 20 Plays pro Anfrage, neueste zuerst, mit Track, Album und Artists), und den öffentlichen Token aus den Einstellungen als `?token=`. Der Token gibt Lesezugriff auf alle Statistiken des Kontos und erscheint deshalb weder im Log noch in Fehlermeldungen.
- **Abruf:** Beim ersten Mal die ganze Historie, danach nur Plays nach dem neuesten bisher importierten (`client = 'Spotify via YourSpotify'`), mit `--all` wieder alles. `gethistory` sortiert fest neueste zuerst, nimmt aber einen Zeitraum (`start`/`end`, beide exklusiv, nur zusammen). Der Import geht deshalb in Zeitfenstern vom ältesten zum neuesten vor und speichert jedes Fenster, bevor er das nächste holt: Die Plays tauchen schon während des Imports im Profil auf, und ein abgebrochener Lauf hinterlässt keine Lücke hinter dem neuesten Listen, der nächste macht dort weiter. Ein Fenster ist 30 Tage lang, nach einem leeren doppelt so lang, höchstens ein Jahr; alles vor 2008 (Spotifys Start) ist ein Fenster. Innerhalb eines Fensters blättert der Import mit `offset`; holt YourSpotify währenddessen neue Plays, rutschen die übrigen eine Seite weiter, doppelt Geholtes fällt über den Zeitpunkt heraus. Liefert YourSpotify Plays außerhalb des Fensters (eine Version ohne Zeitraum), holt der Import wie zuvor alles neueste zuerst und speichert es am Ende.
- **Zuordnung:** Ein Play zählt für seinen ersten Artist (bei Spotify der Haupt-Artist), Track und Album heißen wie bei Spotify, `listened_at` ist `played_at` auf die Millisekunde. MusicBrainz-IDs gibt es keine. Spotify-IDs und alle Artists landen in `extra`, benannt wie in ListenBrainz' `additional_info` (`spotify_id`, `spotify_album_id`, `spotify_artist_ids`, `music_service`). Plays ohne bekannten Artist bleiben draußen und werden gezählt.
- **Dubletten:** wie beim Scrobbeln; ein Play, das das Profil schon hat (gleicher Zeitpunkt), wird übersprungen, deshalb kann `--all` gefahrlos wiederholt werden.
- **Regelmäßig:** `--every 15m` lässt das Kommando laufen und alle 15 Minuten nachholen; mit Docker als eigener Dienst `yourspotify` im Compose-Profil gleichen Namens.
- **Verbindungen im Server:** Tabelle `yourspotify_connection` (höchstens eine pro Profil: Adresse, Token, Start, Ende, bisher importiert, letzter Fehler), angelegt in den Einstellungen oder mit `musicbanana yourspotify add`, das den Token vorher ausprobiert. `musicbanana serve` schaut jede Minute nach fälligen Verbindungen (nie gelaufen, oder zuletzt vor 15 Minuten fertig geworden) und nimmt sie sich mit einem einzigen `UPDATE … RETURNING`, sodass auch mehrere Server-Prozesse eine Verbindung nicht doppelt holen; ein Lauf, der seit einem Tag nicht fertig wurde, gilt als abgebrochen. Der Token geht nie an den Browser.

## Dubletten zusammenführen

`musicbanana merge suggest` schlägt Einträge vor, die wie ein anderer aussehen, jeweils mit dem Befehl zum Zusammenführen (vorerst auf der Kommandozeile, im Browser später mit dem Login):

- **gleiche Buchstaben:** nur Groß-/Kleinschreibung, Akzente, Satzzeichen, Leerzeichen, ein führendes „The“ oder „&“ statt „and“ unterscheiden sich („Bjork“/„Björk“, „AC/DC“/„ACDC“).
- **ein Buchstabe Abstand:** ein Buchstabe mehr, weniger, anders oder mit dem Nachbarn vertauscht („Die Aerzte“/„Die Ärzte“). Erst ab sechs Buchstaben, weil es darunter zu oft ein anderes Wort ist („Blur“/„Blue“), und nie bei verschiedenen Ziffern („Kapitel 1“/„Kapitel 2“).
- **Version:** derselbe Titel ohne Zusatz in Klammern, nach einem Gedankenstrich oder „feat.“ („Unrockbar (Live)“). Das kann eine andere Aufnahme sein, deshalb stehen diese Vorschläge zuletzt.

Der Eintrag mit weniger Listens geht in den mit mehr, eine Version in den Titel ohne Zusatz. Alben und Stücke werden nur innerhalb eines Artists verglichen; ein Artist-Merge nimmt seine Alben und Stücke mit und führt die mit gleichem Titel (in irgendeiner bekannten Schreibweise) mit denen des Ziels zusammen. Ein Merge biegt Listens und Aliase um und setzt `merged_into`, die Rohstrings bleiben unverändert. Rückgängig machen geht noch nicht, daher `--dry-run`.
