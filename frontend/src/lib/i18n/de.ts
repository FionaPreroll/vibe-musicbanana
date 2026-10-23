// Die deutschen Texte, in der Form des englischen Katalogs (en.ts). Angeredet
// wird mit „du“; „Listen“ heißt hier „Wiedergabe“, „Track“ „Titel“.

import type { Messages } from './en.ts';
import { formatNumber as n, plural } from './intl.ts';

const listens = (count: number) =>
	`${n(count)} ${plural(count, { one: 'Wiedergabe', other: 'Wiedergaben' })}`;
const days = (count: number) => `${n(count)} ${plural(count, { one: 'Tag', other: 'Tage' })}`;
/** „im gleichen Zeitraum 2025“, wenn das Jahr noch läuft und nur bis zum selben Tag verglichen wird. */
const sameStretch = (year: number, partial: boolean) =>
	partial ? `im gleichen Zeitraum ${year}` : String(year);

const de: Messages = {
	languageName: 'Deutsch',

	listens,
	plays: listens,
	days,
	duration: (hours, minutes) =>
		hours === 0
			? `${minutes} Min.`
			: minutes === 0
				? `${n(hours)} Std.`
				: `${n(hours)} Std. ${minutes} Min.`,
	// Nach „seit“, also im Dativ: „seit 19 Jahren und 1 Monat“.
	since: {
		units: [
			(c) => `${c} ${plural(c, { one: 'Jahr', other: 'Jahren' })}`,
			(c) => `${c} ${plural(c, { one: 'Monat', other: 'Monaten' })}`,
			(c) => `${c} ${plural(c, { one: 'Woche', other: 'Wochen' })}`,
			(c) => `${c} ${plural(c, { one: 'Tag', other: 'Tagen' })}`,
			(c) => `${c} ${plural(c, { one: 'Stunde', other: 'Stunden' })}`,
			(c) => `${c} ${plural(c, { one: 'Minute', other: 'Minuten' })}`
		],
		two: (first, second) => `${first} und ${second}`
	},

	common: {
		loading: 'Lädt …',
		showMore: 'Mehr anzeigen',
		show: 'Anzeigen',
		undo: 'Rückgängig',
		cancel: 'Abbrechen',
		save: 'Speichern',
		saved: 'Gespeichert.',
		remove: 'Entfernen',
		none: 'Keine.',
		from: 'Von',
		to: 'Bis',
		fewer: 'Weniger',
		more: 'Mehr',
		artists: 'Künstler',
		albums: 'Alben',
		tracks: 'Titel',
		topArtists: 'Top-Künstler',
		topAlbums: 'Top-Alben',
		topTracks: 'Top-Titel',
		quote: (text) => `„${text}“`,
		dateRange: (from, to) => `${from} bis ${to}`
	},

	nav: {
		search: (username) => `${username} durchsuchen`,
		searchLabel: 'Künstler, Alben und Titel dieses Profils durchsuchen',
		searchKey: 'Mit / geht’s zur Suche',
		menu: 'Menü',
		status: 'Status',
		merges: 'Zusammenführen',
		settings: 'Einstellungen',
		logOut: 'Abmelden',
		logIn: 'Anmelden',
		darkMode: 'Dunkles Design',
		toLight: 'Zum hellen Design wechseln',
		toDark: 'Zum dunklen Design wechseln'
	},

	errors: {
		backHome: 'Zurück zur Startseite',
		noProfile: 'Dieses Profil gibt es nicht.',
		noEntity: {
			artist: 'Dieses Profil oder diesen Künstler gibt es nicht.',
			album: 'Dieses Profil oder dieses Album gibt es nicht.',
			track: 'Dieses Profil oder diesen Titel gibt es nicht.'
		},
		noOwnProfile: 'Du hast kein Profil mit dieser Adresse.',
		invalidPeriod: 'Ungültiger Zeitraum',
		mergesForAdmins: 'Zusammenführen dürfen nur Admins.',
		statusForAdmins: 'Die Statusseite ist nur für Admins.'
	},

	home: {
		tagline: 'Ein freier Ort für deinen Hörverlauf.',
		youFollow: 'Du folgst',
		asked: 'Angefragt, noch keine Antwort',
		allProfiles: 'Alle Profile',
		profiles: 'Profile',
		noProfiles: 'Noch keine Profile zu sehen.'
	},

	login: {
		title: 'Anmelden',
		login: 'Benutzername oder E-Mail',
		password: 'Passwort',
		busy: 'Anmelden …',
		submit: 'Anmelden',
		help: 'Das Passwort aus dem alten musicbanana gilt weiter. Neue Konten legt man auf dem Server mit {create} an, und ein vergessenes Passwort ersetzt man dort mit {reset}.',
		usernameArg: '<benutzername>'
	},

	visibility: {
		public: 'Öffentlich',
		followers: 'Nur Follower',
		private: 'Privat',
		privateTitle: 'Nur du siehst dieses Profil',
		followersTitle: 'Du und deine Follower seht dieses Profil'
	},

	follow: {
		ask: 'Folgen anfragen',
		follow: 'Folgen',
		asked: (username) => `Bei ${username} angefragt`,
		takeBack: 'Zurückziehen',
		following: 'Du folgst',
		unfollow: 'Nicht mehr folgen',
		confirmStop: (username) =>
			`Nicht mehr folgen? Du siehst dieses Profil erst wieder, wenn ${username} erneut zustimmt.`,
		goToProfile: 'Zum Profil',
		followersOnly: (username) =>
			`${username} zeigt dieses Profil nur Followern. Es öffnet sich für dich, sobald ${username} deine Anfrage annimmt.`
	},

	source: {
		label: 'Quelle',
		from: 'Quelle',
		all: 'Alle Quellen',
		unknown: 'Unbekannter Player',
		oldMusicbanana: 'Altes musicbanana'
	},

	banana: {
		lives: (since) => `musicbanana lebt schon seit ${since}`,
		python: (since) => `ist seit ${since} eine glückliche Python`,
		elephpant: (since) => `ist seit ${since} ein glücklicher ElePHPant`,
		rustacean: (since) => `ist seit ${since} ein glücklicher Rustacean`,
		sinceTitle: (when) => `seit ${when}`
	},

	chart: {
		nothing: 'Hier ist noch nichts.'
	},

	onThisDay: {
		heading: 'An diesem Tag',
		sub: (day) => `Der ${day} in früheren Jahren`,
		yearsAgo: (years) => (years === 1 ? 'Vor einem Jahr' : `Vor ${n(years)} Jahren`),
		fewerYears: 'Weniger Jahre',
		allYears: (years) => `Alle ${n(years)} Jahre`
	},

	catalogPicker: {
		pickAnother: 'Anderen Eintrag wählen',
		placeholder: 'Name oder #ID'
	},

	clock: {
		heading: 'Höruhr',
		sub: (zone) => `Nach Wochentag und Uhrzeit, Zeitzone ${zone}`,
		busiestDay: 'Stärkster Tag',
		busiestHour: 'Stärkste Stunde',
		share: (percent) => `${percent} der Wiedergaben`,
		label: (day, hours) =>
			`Wiedergaben nach Wochentag und Uhrzeit; die meisten am ${day} und um ${hours}`,
		fewer: 'Weniger',
		more: 'Mehr Wiedergaben'
	},

	period: {
		label: 'Zeitraum',
		allTime: 'Gesamt',
		today: 'Heute',
		lastDays: (count) => `Letzte ${n(count)} Tage`,
		since: (day) => `Seit ${day}`,
		until: (day) => `Bis ${day}`,
		custom: 'Anderer Zeitraum'
	},

	monthCurve: {
		label: (from, to) => `Wiedergaben pro Monat von ${from} bis ${to}`,
		gap: (after, before) => `Keine Wiedergaben zwischen ${after} und ${before}`
	},

	week: {
		heading: 'Diese Woche',
		listens: 'Wiedergaben',
		compare: (diff, lastWeek, partial) => {
			const then = partial ? ' um diese Zeit' : '';
			if (lastWeek === 0)
				return partial ? 'letzte Woche um diese Zeit noch keine' : 'letzte Woche keine';
			if (diff === 0) return `so viele wie letzte Woche${then}`;
			return `${n(Math.abs(diff))} ${diff > 0 ? 'mehr' : 'weniger'} als letzte Woche${then}`;
		},
		daysWithListens: 'Tage mit Musik',
		of: (total) => `von ${n(total)}`,
		streak: 'Serie',
		inARowSince: (day) => `am Stück seit dem ${day}`,
		noneYesterday: 'gestern und heute noch nichts gehört',
		longestStreak: 'Längste Serie',
		on: (day) => `am ${day}`,
		fromTo: (from, to) => `vom ${from} bis ${to}`,
		thisWeek: 'Diese Woche',
		lastWeek: 'Letzte Woche',
		stillToCome: 'kommt noch',
		artists: 'Künstler der Woche',
		firstTime: (count) => `${n(count)} zum ersten Mal gehört`,
		noneYet: 'Diese Woche noch nichts gehört.',
		new: 'neu',
		lastWeekCount: (count) => `${n(count)} letzte Woche`
	},

	lost: {
		heading: 'Fundbüro',
		notSince: (day) =>
			day
				? `Oft gehört, aber seit dem ${day} nicht mehr`
				: 'Oft gehört, aber schon lange nicht mehr',
		nothing: 'Gerade ist nichts verschollen.',
		lastHeardOn: (day) => `Zuletzt gehört am ${day}`,
		lastHeard: (ago) => `zuletzt gehört ${ago}`,
		notInterested: 'Interessiert mich nicht mehr',
		dismiss: (name) => `${name} ausblenden`,
		dismissed: (name, artist) =>
			artist ? `${name} samt Titeln ausgeblendet.` : `${name} ausgeblendet.`,
		hideDismissed: 'Ausgeblendete zuklappen',
		dismissedCount: (count) => `Ausgeblendet (${n(count)})`,
		artistAndTracks: 'Künstler samt Titeln',
		showAgain: 'Wieder zeigen',
		nothingDismissed: 'Nichts ausgeblendet.'
	},

	artistYears: {
		heading: 'Top-Künstler im Lauf der Jahre',
		sub: 'Die meistgehörten Künstler jedes Jahres. Eine Linie verbindet einen Künstler von einem Jahr zum nächsten.',
		longest: 'Am längsten oben:',
		region: 'Top-Künstler jedes Jahres',
		gap: (after, before) =>
			after === before
				? `Keine Wiedergaben im Jahr ${after}`
				: `Keine Wiedergaben von ${after} bis ${before}`,
		chartsOf: (year) => `Charts ${year}`,
		topOf: (year) => `Top-Künstler ${year}`,
		place: (name, rank, year, listens) => `${name}, Platz ${rank} im Jahr ${year}, ${listens}`,
		tip: (rank, year) => `Platz ${rank} im Jahr ${year}`
	},

	profile: {
		span: (from, to) => `vom ${from} bis ${to}`,
		nowPlaying: 'Läuft gerade',
		inReview: (year) => `Jahresrückblick ${year} →`,
		lastOfYear: (year) => `Letzte Wiedergaben ${year}`,
		lastUpTo: (day) => `Letzte Wiedergaben bis ${day}`,
		recent: 'Zuletzt gehört',
		noListens: 'Keine Wiedergaben.',
		older: 'Ältere Wiedergaben'
	},

	entity: {
		kinds: { artist: 'Künstler', album: 'Album', track: 'Titel' },
		musicBrainzOf: (i, count) => `MusicBrainz ${i} von ${count}`,
		by: 'von {artist}',
		listens: 'Wiedergaben',
		firstListen: 'Zuerst gehört',
		lastListen: 'Zuletzt gehört',
		perMonth: 'Wiedergaben pro Monat',
		notHeard: 'In diesem Profil nie gehört.',
		heavy: 'Hochphasen',
		onAlbums: 'Auf Alben'
	},

	search: {
		title: 'Suche',
		placeholder: 'Künstler, Album oder Titel',
		label: (username) => `Künstler, Alben und Titel von ${username} durchsuchen`,
		nothing: (q) => `Nichts Gehörtes passt zu „${q}“.`,
		hint: 'Jedes Wort zählt, egal ob groß oder klein, mit oder ohne Akzente: „ärzte unrockbar“ findet den Titel.'
	},

	review: {
		title: (year) => `Jahresrückblick ${year}`,
		crumb: 'Jahresrückblick',
		soFar: 'bis heute',
		otherYears: 'Andere Jahre',
		noListens: (year, partial) =>
			partial ? `Bisher keine Wiedergaben im Jahr ${year}.` : `Keine Wiedergaben im Jahr ${year}.`,
		compare: (change, lastYear, year, partial, percent) => {
			if (lastYear === 0) return partial ? `${year} im gleichen Zeitraum keine` : `${year} keine`;
			if (change === 0) return `so viele wie ${sameStretch(year, partial)}`;
			return `${percent} ${change > 0 ? 'mehr' : 'weniger'} als ${sameStretch(year, partial)}`;
		},
		newAndTracks: (fresh, tracks) => `${n(fresh)} neu, ${n(tracks)} Titel`,
		daysWithListens: 'Tage mit Musik',
		busiestMonth: 'Stärkster Monat',
		busiestDay: 'Stärkster Tag',
		longestSession: 'Längste Session',
		listensOn: (listens, day) => `${listens} am ${day}`,
		discovered: (year) => `Entdeckungen ${year}`,
		noDiscoveries: 'Keine Künstler zum ersten Mal gehört.',
		discoveries: (count, more) =>
			`${n(count)} Künstler zum ersten Mal gehört${more ? ', hier die meistgehörten' : ''}`,
		since: (day) => `seit dem ${day}`,
		risers: 'Größte Aufsteiger',
		noRisers: (year, partial) =>
			`Kein schon bekannter Künstler kam auf mehr Wiedergaben als ${sameStretch(year, partial)}.`,
		risersSub: (year, partial) =>
			`Schon bekannte Künstler mit dem größten Zuwachs gegenüber ${partial ? `dem gleichen Zeitraum ${year}` : year}`,
		back: 'zurück',
		notHeardIn: (year, partial) =>
			partial ? `Im gleichen Zeitraum ${year} nicht gehört` : `${year} nicht gehört`,
		places: (before, year, partial) =>
			`Platz in den Künstler-Charts ${before}${partial ? ' (gleicher Zeitraum)' : ''} und ${year}`,
		mostly:
			'Vor allem {artists}. Eine Session läuft weiter, solange die nächste Wiedergabe höchstens 30 Minuten nach dem Ende der vorigen beginnt.'
	},

	settings: {
		title: 'Einstellungen',
		profiles: {
			heading: 'Profile',
			intro:
				'Jedes Profil hat eigene Wiedergaben und Charts, zum Beispiel eins pro Player oder pro Person.',
			editHistory: 'Hörverlauf bearbeiten',
			whoSees: (name) => `Wer ${name} sieht`,
			nameOf: (slug) => `Name von ${slug}`,
			rename: 'Umbenennen',
			new: 'Neues Profil',
			address: 'Adresse',
			addressRule: 'Kleinbuchstaben, Ziffern und Bindestriche',
			name: 'Name',
			whoSeesIt: 'Wer es sieht',
			addressMakes: (path) => `Daraus wird die Adresse ${path}.`,
			create: 'Profil anlegen'
		},
		tokens: {
			heading: 'Scrobble-Tokens',
			intro:
				'Ein Player scrobbelt mit einem Token, und das Token bestimmt, in welchem Profil die Wiedergaben landen. In Navidrome gehört es in die persönlichen Einstellungen, zu ListenBrainz.',
			created:
				'Das Token für {label}. Es wird nur dieses eine Mal angezeigt, also kopier es gleich in den Player:',
			done: 'Fertig',
			none: 'Noch keine Tokens.',
			label: 'Bezeichnung',
			profile: 'Profil',
			lastUsed: 'Zuletzt benutzt',
			never: 'nie',
			revoke: 'Widerrufen',
			confirmRevoke: (label) =>
				`Token „${label}“ widerrufen? Der Player kann damit dann nicht mehr scrobbeln.`,
			create: 'Token erstellen'
		},
		yourspotify: {
			heading: 'Spotify über YourSpotify',
			intro:
				'{yourspotify} speichert den Verlauf eines Spotify-Kontos. Mit einem Profil verbunden, übernimmt musicbanana erst den ganzen Verlauf und danach alle 15 Minuten die neuen Wiedergaben. Pro Profil geht ein Spotify-Konto. Hat YourSpotify inzwischen einen älteren Spotify-Export eingelesen, holt {fetchAll} dessen Wiedergaben; was schon da ist, wird übersprungen.',
			soFar: (listens) => `bisher ${listens}`,
			fetchAll: 'Alles neu abholen',
			fetchAllTitle:
				'Den ganzen Verlauf noch einmal abholen, für ältere Wiedergaben, die YourSpotify nachträglich bekommen hat',
			confirmDisconnect: (profile) =>
				`Keine Wiedergaben mehr von YourSpotify nach ${profile} übernehmen? Die bisherigen bleiben.`,
			confirmRefetch: (profile) =>
				`Den ganzen Verlauf von YourSpotify noch einmal nach ${profile} abholen? Was schon da ist, wird übersprungen.`,
			cliOnly:
				'Dieser Server nimmt YourSpotify-Verbindungen nur über die Kommandozeile an. Wer ihn betreut, kann mit {variable} Adressen für diese Seite freigeben.',
			profile: 'Profil',
			address: 'Adresse der YourSpotify-API',
			token: 'Öffentliches Token',
			checking: 'Wird geprüft …',
			connect: 'Verbinden',
			addressHelp: (addresses) =>
				`Gemeint ist die API von YourSpotify (ihr API_ENDPOINT, nicht die Weboberfläche), so wie der musicbanana-Server sie erreicht: ${addresses} oder ein Pfad darunter. Das öffentliche Token steht in den Einstellungen von YourSpotify.`
		},
		followers: {
			heading: 'Follower',
			intro:
				'Öffentlichen Profilen kann folgen, wer angemeldet ist. Ein Profil nur für Follower sehen die, denen du zugestimmt hast; entfernst du jemanden, ist es für die Person wieder verborgen.',
			none: 'Deinen Profilen folgt noch niemand.',
			asks: (since) => `{user} möchte {profile} folgen, angefragt am ${since}`,
			follows: (since) => `{user} folgt {profile} seit dem ${since}`,
			yes: 'Annehmen',
			no: 'Ablehnen',
			confirmNo: (username) => `Die Anfrage von ${username} ablehnen?`,
			confirmRemove: (username) => `${username} als Follower entfernen?`
		},
		time: {
			heading: 'Zeit',
			intro:
				'Tage, Wochen, Jahre und die Höruhr richten sich nach deiner Zeitzone, auf deinen Profilen genauso wie auf allen anderen. Ohne Angabe gilt die Zeitzone dieses Browsers.',
			zone: 'Zeitzone',
			browserZone: (zone) => `Die dieses Browsers (${zone})`,
			weekStart: 'Wochen beginnen am'
		},
		language: {
			heading: 'Sprache',
			intro: 'Die Sprache dieser Seiten. Ohne Auswahl gilt die Sprache des Browsers.',
			browser: (name) => `Die des Browsers (${name})`
		},
		streaks: {
			heading: 'Serien',
			text: 'Serien anzeigen, also die Tage am Stück mit Wiedergaben, unter „Diese Woche“ auf den Profilseiten. Ausgeschaltet siehst du sie auf keinem Profil, und niemand sieht die Serien deiner Profile.'
		},
		username: {
			heading: 'Benutzername',
			intro:
				'Er steht in der Adresse deiner Profile, und du meldest dich damit an, genau wie mit deiner E-Mail-Adresse. Links mit einem alten Namen führen zum neuen, und niemand sonst kann den alten übernehmen. Scrobble-Tokens und YourSpotify-Verbindungen bleiben, wie sie sind.',
			label: 'Benutzername',
			rename: 'Umbenennen',
			renamed: (old) => `Umbenannt. Links mit ${old} führen jetzt zu {link}.`
		},
		email: {
			heading: 'E-Mail-Adresse',
			intro:
				'Damit meldest du dich an, genau wie mit deinem Benutzernamen. musicbanana verschickt keine Mails, deshalb wird sie nicht überprüft.',
			new: 'Neue Adresse',
			password: 'Dein Passwort',
			changed: (address) => `Geändert auf ${address}.`,
			change: 'Adresse ändern'
		},
		password: {
			heading: 'Passwort',
			current: 'Aktuelles Passwort',
			new: 'Neues Passwort',
			again: 'Neues Passwort wiederholen',
			differ: 'Die beiden neuen Passwörter stimmen nicht überein.',
			changed:
				'Geändert. Andere Browser, die mit diesem Konto angemeldet waren, sind jetzt abgemeldet.',
			change: 'Passwort ändern'
		},
		data: {
			heading: 'Deine Daten',
			intro:
				'Eine JSON-Datei mit allem, was musicbanana über dein Konto speichert: das Konto, deine Profile mit allen Wiedergaben (im Importformat von ListenBrainz) samt Papierkorb, wem du folgst und wer dir folgt, Scrobble-Tokens, YourSpotify-Verbindungen und Anmeldungen. Passwörter und Tokens bleiben draußen.',
			download: 'Daten herunterladen'
		},
		delete: {
			heading: 'Profil oder Konto löschen',
			text: 'Wende dich an die Person, die diesen Server betreut. Mit einem Profil verschwinden seine Wiedergaben, Scrobble-Tokens, die YourSpotify-Verbindung und die Follower, mit einem Konto alle seine Profile und Anmeldungen. Das lässt sich nicht rückgängig machen.'
		}
	},

	connection: {
		starts: 'Startet innerhalb einer Minute.',
		importing: (since) => `Import läuft seit ${since} …`,
		failed: (at, error) => `Fehlgeschlagen am ${at}: ${error}`,
		upToDate: (at) => `Aktuell, Stand ${at}.`,
		refetching: (found, added, upTo) =>
			`Alles wird neu abgeholt: bisher ${listens(found)}, davon ${n(added)} neu${upTo ? `, bis zum ${upTo}` : ''} …`,
		refetchStopped: (added, upTo) =>
			`Neu abholen unterbrochen${upTo ? `, fertig bis zum ${upTo}` : ''}, bisher ${n(added)} neu; es geht in der nächsten Runde weiter.`,
		refetchAfterImport: 'Holt alles neu ab, sobald dieser Import fertig ist.',
		refetchSoon: 'Holt innerhalb einer Minute alles neu ab.',
		refetched: (at, found, added, there, leftOut) =>
			`Am ${at} alles neu abgeholt: ${listens(found)}, ${n(added)} neu, ${n(there)} schon vorhanden` +
			(leftOut > 0
				? `, ${n(leftOut)} ausgelassen, weil YourSpotify keinen ihrer Künstler kennt`
				: '') +
			'.'
	},

	history: {
		title: 'Hörverlauf bearbeiten',
		intro:
			'von {profile}. Wiedergaben im Papierkorb verschwinden aus den Charts und von allen Seiten und bleiben dort, bis du sie wiederherstellst oder den Papierkorb leerst. Schickt ein Player oder ein Import sie noch einmal, kommen sie trotzdem nicht zurück.',
		listens: 'Wiedergaben',
		trash: 'Papierkorb',
		words: 'Suchbegriffe',
		placeholder: 'Künstler, Titel oder Album',
		source: 'Quelle',
		clear: 'Zurücksetzen',
		inTrash: (count) => `${listens(count)} im Papierkorb.`,
		total: (count, filtered) =>
			filtered
				? `${listens(count)} ${plural(count, { one: 'passt', other: 'passen' })}.`
				: `${listens(count)}.`,
		selected: (count) => `${n(count)} ausgewählt.`,
		restoreSelected: 'Auswahl wiederherstellen',
		restoreAll: 'Alle wiederherstellen',
		deleteSelected: 'Auswahl endgültig löschen',
		emptyTrash: 'Papierkorb leeren',
		moveSelected: 'Auswahl in den Papierkorb',
		moveAll: (count) => `Alle ${n(count)} in den Papierkorb`,
		moved: (count) => `${listens(count)} in den Papierkorb verschoben.`,
		confirmMoveAll: (count) =>
			`${plural(count, { one: 'Die passende', other: `Alle ${n(count)} passenden` })} ${plural(count, { one: 'Wiedergabe', other: 'Wiedergaben' })} in den Papierkorb verschieben?`,
		restored: (restored, kept) =>
			`${listens(restored)} wiederhergestellt.` +
			(kept
				? ` ${listens(kept)} ${plural(kept, { one: 'bleibt', other: 'bleiben' })} im Papierkorb, weil das Profil zu dieser Zeit inzwischen eine andere Wiedergabe hat.`
				: ''),
		confirmDelete: (count) =>
			`${listens(count)} endgültig löschen? Das lässt sich nicht rückgängig machen.`,
		deleted: (count) => `${listens(count)} endgültig gelöscht.`,
		trashEmpty: 'Der Papierkorb ist leer.',
		noListens: 'Keine Wiedergaben.',
		selectAll: 'Alle angezeigten auswählen',
		select: (artist, track, at) => `${artist} – ${track}, ${at} auswählen`,
		time: 'Zeit',
		listen: 'Wiedergabe',
		trashed: 'Verschoben'
	},

	merges: {
		title: 'Zusammenführen',
		intro:
			'Zusammenführen legt zwei Einträge des Katalogs zusammen, die dasselbe in verschiedener Schreibweise sind, und zwar für alle Profile. Die Wiedergaben behalten, was die Player geschickt haben, spätere Wiedergaben in einer der beiden Schreibweisen zählen für den Eintrag, der bleibt, und jede Zusammenführung lässt sich unten im Protokoll rückgängig machen.',
		kind: { artist: 'Künstler', release: 'Album', recording: 'Titel' },
		kinds: { artist: 'Künstler', release: 'Alben', recording: 'Titel' },
		likeness: {
			same_letters: [
				'Gleiche Buchstaben',
				'Nur Groß- und Kleinschreibung, Akzente, Satzzeichen, Leerzeichen, ein „The“ am Anfang oder „&“ statt „and“ unterscheiden sich.'
			],
			one_letter: [
				'Ein Buchstabe anders',
				'Ein Buchstabe zu viel, zu wenig, ein anderer oder zwei vertauscht.'
			],
			version: [
				'Andere Fassung?',
				'Gleicher Titel bis auf einen Zusatz wie „(Live)“, „- Radio Edit“ oder „feat. …“. Live-Fassungen und das Original hält man besser auseinander.'
			]
		},
		merged: (kind, from, into, count) =>
			`${kind} „${from}“ in „${into}“ zusammengeführt: ${listens(count)} übertragen.`,
		hidden: 'Ausgeblendet. Der Vorschlag kommt erst wieder, wenn du ihn wieder einblendest.',
		suggestedAgain: (a, b) => `„${a}“ und „${b}“ werden wieder vorgeschlagen.`,
		undid: (id, kind, from) =>
			`Zusammenführung ${id} rückgängig gemacht: ${kind} „${from}“ steht wieder für sich.`,
		suggestions: 'Vorschläge',
		noneLeft: 'Keine Doppelgänger mehr.',
		firstOf: (count, total) => `Die ersten ${n(count)} von ${n(total)}, die sichersten zuerst.`,
		count: (count) => `${n(count)} Doppelgänger, die sichersten zuerst.`,
		withinArtist:
			'Alben und Titel werden nur innerhalb eines Künstlers verglichen. Wer zuerst doppelte Künstler zusammenführt, führt damit auch deren gleichnamige Alben und Titel zusammen.',
		closeHidden: 'Ausgeblendete zuklappen',
		hiddenCount: (count) => `${n(count)} ausgeblendet`,
		pair: (a, aId, b, bId) => `„${a}“ (#${aId}) und „${b}“ (#${bId})`,
		hiddenAt: (at) => `ausgeblendet am ${at}`,
		suggestAgain: 'Wieder vorschlagen',
		into: 'in',
		mergeEllipsis: 'Zusammenführen …',
		otherWay: 'Andersherum …',
		otherWayTitle: (from, into) =>
			`Stattdessen „${from}“ behalten und „${into}“ darin aufgehen lassen`,
		hide: 'Ausblenden',
		hideTitle: 'Das sind verschiedene; nicht mehr vorschlagen',
		byHand: 'Von Hand zusammenführen',
		anyTwo: (kinds) => `Zwei beliebige ${kinds}, auch solche, die die Vorschläge nicht finden.`,
		fromLabel: 'Zusammenführen (verschwindet)',
		intoLabel: 'In (bleibt)',
		swap: 'Tauschen',
		preview: 'Vorschau',
		wouldMerge: (kind) => `${kind} {from} würde in {into} aufgehen.`,
		countFor: (count, spellings, into) =>
			`${listens(count)} und ${n(spellings)} ${plural(spellings, { one: 'Schreibweise', other: 'Schreibweisen' })} zählen ab dann für „${into}“.`,
		albumsMoved: (moved, merged) =>
			`Alben: ${n(moved)} übertragen, ${n(merged)} mit einem gleichnamigen zusammengeführt.`,
		tracksMoved: (moved, merged) =>
			`Titel: ${n(moved)} übertragen, ${n(merged)} mit einem gleichnamigen zusammengeführt.`,
		toldApart:
			'Laut ihren MusicBrainz-IDs sind das zwei verschiedene. Trotzdem zusammenführen, etwa bei einem anderen Namen eines Künstlers oder einem Titel, den MusicBrainz doppelt führt.',
		merge: 'Zusammenführen',
		log: 'Protokoll',
		logIntro:
			'Alle Zusammenführungen, von hier, von der Kommandozeile und aus {editions}, die neuesten zuerst. Rückgängig machen stellt wieder her, was sich geändert hat; spätere Zusammenführungen derselben Einträge müssen vorher rückgängig gemacht werden.',
		noMerges: 'Noch keine Zusammenführungen.',
		undone: 'rückgängig gemacht',
		undoEllipsis: 'Rückgängig …',
		wouldUndo: (id, kind, from, restored, kept) =>
			`Würde Zusammenführung ${id} rückgängig machen: ${kind} „${from}“ steht wieder für sich, ${n(restored)} ${plural(restored, { one: 'Änderung wird', other: 'Änderungen werden' })} zurückgenommen.` +
			(kept
				? ` ${n(kept)} ${plural(kept, { one: 'Zeile bleibt, wie sie ist, weil sie sich', other: 'Zeilen bleiben, wie sie sind, weil sie sich' })} seitdem wieder geändert ${plural(kept, { one: 'hat', other: 'haben' })}.`
				: ''),
		undoMerge: 'Zusammenführung rückgängig machen'
	},

	status: {
		title: 'Status',
		refresh: 'Aktualisiert sich alle 10 Sekunden.',
		build: 'Build',
		version: 'Version',
		debugBuild: '(Debug-Build)',
		commit: 'Commit',
		unknownCommit: 'unbekannt (lokaler Build)',
		started: 'Gestartet',
		uptime: 'Läuft seit',
		process: 'Prozess',
		cpu: (percent, seconds) => `${percent} % eines Kerns (${seconds} s insgesamt)`,
		memory: 'Arbeitsspeicher',
		memoryValue: (now, peak) => `${now}, höchstens ${peak}`,
		threads: 'Threads',
		machine: 'Rechner',
		machineValue: (cores, load) => `${n(cores)} Kerne, Last ${load}`,
		machineMemory: 'Speicher des Rechners',
		free: (free, total, limit) => `${free} frei von ${total}${limit ? `, Limit ${limit}` : ''}`,
		database: 'Datenbank',
		size: 'Größe',
		migration: 'Migration',
		connections: 'Verbindungen',
		connectionsValue: (busy, idle, total) =>
			`${n(busy)} belegt, ${n(idle)} frei im Pool; ${total} zur Datenbank`,
		cacheHits: 'Cache-Treffer',
		lastDay: 'Wiedergaben der letzten 24 Stunden',
		yourspotify: 'YourSpotify-Verbindungen',
		lastRound: (at) =>
			at
				? `Zuletzt nach fälligen Verbindungen gesehen: ${at}.`
				: 'Noch nicht nach fälligen Verbindungen gesehen.',
		routes: 'Antwortzeiten der API seit dem Start',
		routesIntro:
			'Nach Route unter /api, die mit der meisten Zeit insgesamt zuerst; Median und 95. Perzentil jeweils über die letzten 500 Anfragen.',
		route: 'Route',
		requests: 'Anfragen',
		mean: 'Mittel',
		median: 'Median',
		max: 'Max.',
		tables: 'Tabellen',
		table: 'Tabelle',
		rows: 'Zeilen (ca.)',
		uptimeValue: (d, h, m, s) =>
			d > 0 ? `${d} T. ${h} Std.` : h > 0 ? `${h} Std. ${m} Min.` : `${m} Min. ${s} s`
	},

	serverErrors: {
		notLoggedIn: () => 'Nicht angemeldet.',
		adminsOnly: () => 'Das ist nur für Admins.',
		tooManyLogins: (minutes) =>
			`Zu viele fehlgeschlagene Anmeldungen. Versuch es in ${minutes} Minuten noch einmal.`,
		wrongLogin: () => 'Benutzername oder Passwort stimmt nicht.',
		currentPasswordWrong: () => 'Das aktuelle Passwort stimmt nicht.',
		passwordTooShort: (min) => `Das neue Passwort braucht mindestens ${min} Zeichen.`,
		passwordWrong: () => 'Das Passwort stimmt nicht.',
		profileNeedsName: () => 'Das Profil braucht einen Namen.',
		profileExists: (slug) => `Du hast schon ein Profil „${slug}“.`,
		tokenNeedsLabel: () => 'Gib an, wofür das Token ist, zum Beispiel Navidrome.',
		yourSpotifyCliOnly: () =>
			'Dieser Server nimmt YourSpotify-Verbindungen nur über die Kommandozeile an.',
		yourSpotifyOnly: (addresses) =>
			`Dieser Server nimmt YourSpotify nur unter ${addresses.replaceAll(' or ', ' oder ')} an.`,
		yourSpotifyConnected: () =>
			'Das Profil ist schon mit YourSpotify verbunden. Entferne zuerst die bestehende Verbindung.',
		yourSpotifyTokenEmpty: () => 'Das YourSpotify-Token fehlt.',
		yourSpotifyTokenRefused: (address) =>
			`YourSpotify unter ${address} nimmt das Token nicht an. Das öffentliche Token steht in den Einstellungen von YourSpotify.`,
		yourSpotifyAnswered: (address, status) =>
			`YourSpotify unter ${address} hat auf die Frage nach dem Verlauf mit ${status} geantwortet.`,
		yourSpotifyUnreachable: (address, cause) =>
			`YourSpotify unter ${address} ist nicht erreichbar: ${cause}`,
		notHttp: (address) => `${address} ist keine http(s)-Adresse.`,
		notAnAddress: (address) => `${address} ist keine gültige Adresse.`,
		badEmail: (address) => `„${address}“ sieht nicht nach einer E-Mail-Adresse aus.`,
		emailTaken: (address) => `Ein anderes Konto hat schon die Adresse ${address}.`,
		badUsername: (name) =>
			`„${name}“ geht nicht als Benutzername: erlaubt sind bis zu 32 Buchstaben, Ziffern, Bindestriche, Punkte und Unterstriche, am Anfang ein Buchstabe oder eine Ziffer.`,
		usernameTaken: (name) => `Der Benutzername „${name}“ ist schon vergeben.`,
		badSlug: (slug) =>
			`„${slug}“ geht nicht als Profiladresse: erlaubt sind bis zu 32 Kleinbuchstaben, Ziffern und Bindestriche.`,
		slugReserved: (slug) => `„${slug}“ ist für die Seiten eines Profils reserviert.`,
		ownProfile: () => 'Das ist dein eigenes Profil.',
		pickListens: () => 'Wähle Wiedergaben aus oder grenze den Verlauf zuerst ein.',
		unknownTimeZone: (zone) => `Unbekannte Zeitzone: ${zone}`,
		unknownLanguage: (code) => `Unbekannte Sprache: ${code}`,
		internal: () => 'Interner Serverfehler.',
		notFound: () => 'Nicht gefunden.'
	}
};

export default de;
