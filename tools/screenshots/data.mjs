// Made-up listening histories for the screenshots: real artists and songs,
// invented plays. Everything is drawn from a seeded random generator, so two
// runs on the same day make the same data; only the dates move with "now".

const DAY = 24 * 60 * 60 * 1000;

// The accounts, all with the same password.
export const PASSWORD = 'screenshots';
export const USERS = {
	fiona: { email: 'fiona@example.org', admin: true, history: { years: 3, perDay: 9, seed: 1 } },
	alex: {
		email: 'alex@example.org',
		admin: false,
		history: { years: 2, perDay: 5, seed: 2, streakDays: 20 }
	}
};

/** Small, fast, seeded PRNG (mulberry32). */
export function random(seed) {
	let a = seed >>> 0;
	return () => {
		a = (a + 0x6d2b79f5) >>> 0;
		let t = a;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

/** A stable UUID-shaped id from a name: MusicBrainz never issued these. */
export function fakeMbid(name) {
	let h = [0x811c9dc5, 0x01000193, 0x2545f491, 0x9e3779b9];
	for (const ch of name) {
		const c = ch.codePointAt(0);
		h = h.map((x, i) => Math.imul(x ^ (c + i * 131), 0x01000193 + i * 2) >>> 0);
	}
	const hex = h.map((x) => x.toString(16).padStart(8, '0')).join('');
	return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-4${hex.slice(13, 16)}-a${hex.slice(17, 20)}-${hex.slice(20, 32)}`;
}

// Albums with `cover: false` have no MusicBrainz ID, so they show the placeholder.
export const CATALOG = [
	{
		artist: 'Die Ärzte',
		albums: [
			{
				title: 'Geräusch',
				color: '#e11d48',
				tracks: ['Unrockbar', 'Deine Schuld', 'Dein Vampyr', 'Nichts in der Welt']
			},
			{
				title: 'Jazz ist anders',
				color: '#0f766e',
				tracks: ['Junge', 'Lied vom Scheitern', 'Himmelblau']
			}
		]
	},
	{
		artist: 'Björk',
		albums: [
			{
				title: 'Debut',
				color: '#a16207',
				tracks: ['Human Behaviour', 'Venus as a Boy', 'Big Time Sensuality']
			},
			{ title: 'Homogenic', color: '#7c3aed', tracks: ['Jóga', 'Bachelorette', 'Hunter'] }
		]
	},
	{
		artist: 'Wir sind Helden',
		albums: [
			{
				title: 'Die Reklamation',
				color: '#ea580c',
				tracks: ['Guten Tag', 'Denkmal', 'Müssen nur wollen']
			}
		]
	},
	{
		artist: 'Radiohead',
		albums: [
			{
				title: 'OK Computer',
				color: '#0369a1',
				tracks: ['Paranoid Android', 'Karma Police', 'No Surprises']
			},
			{ title: 'In Rainbows', color: '#16a34a', tracks: ['Reckoner', 'Nude', '15 Step'] }
		]
	},
	{
		artist: 'Portishead',
		albums: [{ title: 'Dummy', color: '#334155', tracks: ['Sour Times', 'Glory Box', 'Roads'] }]
	},
	{
		artist: 'Tocotronic',
		albums: [
			{
				title: 'Pure Vernunft darf niemals siegen',
				color: '#be185d',
				tracks: [
					'Aber hier leben, nein danke',
					'Pure Vernunft darf niemals siegen',
					'Gegen den Strich'
				]
			}
		]
	},
	{
		artist: 'Element of Crime',
		albums: [
			{
				title: 'Weißes Papier',
				color: '#57534e',
				tracks: ['Weißes Papier', 'Damals hinterm Mond', 'Ich wollt mich nur erkundigen']
			}
		]
	},
	{
		artist: 'Kraftwerk',
		albums: [
			{
				title: 'Computerwelt',
				color: '#ca8a04',
				tracks: ['Computerliebe', 'Taschenrechner', 'Computerwelt']
			}
		]
	},
	{
		artist: 'Massive Attack',
		albums: [
			{ title: 'Mezzanine', color: '#111827', tracks: ['Teardrop', 'Angel', 'Inertia Creeps'] }
		]
	},
	{
		artist: 'Deichkind',
		albums: [
			{
				title: 'Befehl von ganz unten',
				color: '#dc2626',
				cover: false,
				tracks: ['Leider geil', 'Bück dich hoch']
			}
		]
	},
	{
		artist: 'Robyn',
		albums: [
			{
				title: 'Body Talk',
				color: '#db2777',
				tracks: ['Dancing On My Own', 'Call Your Girlfriend', 'Hang With Me']
			}
		]
	},
	{
		artist: 'Peter Fox',
		albums: [
			{
				title: 'Stadtaffe',
				color: '#65a30d',
				tracks: ['Haus am See', 'Alles neu', 'Schwarz zu Blau']
			}
		]
	},
	{
		artist: 'Bilderbuch',
		albums: [{ title: 'Magic Life', color: '#0891b2', tracks: ['Bungalow', 'Sweetlove', 'Baba'] }]
	},
	{
		artist: 'Sophie Hunger',
		albums: [
			{
				title: '1983',
				color: '#9333ea',
				tracks: ['1983', 'Headlights', 'Leave Me With the Monkeys']
			}
		]
	},
	{
		artist: 'Beatsteaks',
		albums: [
			{
				title: 'Smack Smash',
				color: '#b91c1c',
				tracks: ['Hand in Hand', 'Hello Joe', 'I Don’t Care as Long as You Sing']
			}
		]
	}
];

// Every album with a cover, for writing the cover files.
export const COVERS = CATALOG.flatMap(({ artist, albums }) =>
	albums
		.filter((a) => a.cover !== false)
		.map((a) => ({
			artist,
			title: a.title,
			color: a.color,
			mbid: fakeMbid(`${artist}/${a.title}`)
		}))
);

/**
 * How much a listener likes each artist at a point in time: `x` runs from 0
 * (the first day of the history) to 1 (today). Shapes give the charts some
 * movement: one artist fades out (Lost & Found), others rise late (new in the
 * year review, ↑ in the charts).
 */
const TASTES = {
	fiona: {
		'Die Ärzte': () => 10,
		Björk: (x) => 7 + 2 * Math.sin(x * 9),
		'Wir sind Helden': (x) => 6 - 3 * x,
		Radiohead: (x) => 4 + 3 * x,
		Portishead: (x) => 4 + 2 * Math.cos(x * 7),
		Tocotronic: (x) => 5 * (1 - Math.abs(x - 0.5)),
		'Element of Crime': (x) => (x < 0.35 ? 8 : 0),
		Kraftwerk: () => 2.5,
		'Massive Attack': (x) => (x > 0.3 ? 2.5 : 0.5),
		Deichkind: (x) => (x > 0.6 ? 2 : 0),
		'Peter Fox': (x) => (x > 0.85 ? 3 + 30 * (x - 0.85) : 0.3),
		Bilderbuch: (x) => (x > 0.75 ? 4 : 0),
		'Sophie Hunger': (x) => (x < 0.25 ? 5 : 0)
	},
	alex: {
		'Die Ärzte': () => 6,
		Robyn: () => 8,
		'Massive Attack': () => 5,
		Deichkind: () => 6,
		Beatsteaks: () => 7,
		Radiohead: () => 2,
		Björk: (x) => (x > 0.5 ? 2 : 0),
		Bilderbuch: () => 3
	}
};

const SOURCES = [
	{
		weight: 0.75,
		info: {
			submission_client: 'Navidrome',
			submission_client_version: '0.58.0',
			media_player: 'Supersonic'
		}
	},
	{ weight: 0.25, info: { submission_client: 'Spotify via YourSpotify' } }
];

function pick(rand, items, weight) {
	const total = items.reduce((s, i) => s + weight(i), 0);
	let r = rand() * total;
	for (const item of items) {
		r -= weight(item);
		if (r <= 0) return item;
	}
	return items.at(-1);
}

function durationOf(artist, track) {
	let h = 0;
	for (const ch of artist + track) h = (h * 31 + ch.codePointAt(0)) >>> 0;
	return 150_000 + (h % 180_000); // 2:30 to 5:30
}

/**
 * The listens of one listener as ListenBrainz payload entries, oldest first.
 * `years` back from `now`; the last `streakDays` days all have listens.
 */
export function history(
	user,
	{ now = Date.now(), years = 3, perDay = 9, streakDays = 120, seed = 1 } = {}
) {
	const rand = random(seed);
	const tastes = TASTES[user];
	const artists = CATALOG.filter((a) => tastes[a.artist]);
	const days = Math.round(years * 365);
	const source = SOURCES;
	const listens = [];
	// Berlin is UTC+1 or +2; an hour off at night does not matter here.
	const midnight = Math.floor(now / DAY) * DAY - 60 * 60 * 1000;
	for (let d = days; d >= 0; d--) {
		const x = 1 - d / days;
		const dayStart = midnight - d * DAY;
		const weekday = new Date(dayStart + 12 * 60 * 60 * 1000).getUTCDay();
		if (d > streakDays && rand() < 0.15) continue;
		const quiet = weekday === 0 || weekday === 6 ? 0.7 : 1;
		const count = Math.max(1, Math.round(perDay * quiet * (0.4 + rand() * 1.2)));
		const sessions = 1 + Math.floor(rand() * 3);
		let left = count;
		for (let s = 0; s < sessions && left > 0; s++) {
			const n = s === sessions - 1 ? left : Math.max(1, Math.round(left * rand()));
			left -= n;
			// Mornings, lunch or evenings, mostly evenings.
			// Before 22:00, so that a day's listens stay on that day.
			const hour = Math.min(
				22,
				pick(rand, [7.5, 12.5, 17, 19.5, 21], (h) => (h > 18 ? 3 : h === 17 ? 2 : 1)) +
					rand() * 3 -
					1
			);
			let at = dayStart + hour * 60 * 60 * 1000;
			const src = pick(rand, source, (s) => s.weight).info;
			// A session stays with one or two artists, often a whole album.
			let current = pick(rand, artists, (a) => Math.max(0, tastes[a.artist](x)));
			for (let i = 0; i < n; i++) {
				if (i > 0 && rand() < 0.3)
					current = pick(rand, artists, (a) => Math.max(0, tastes[a.artist](x)));
				const album = current.albums[Math.floor(rand() * current.albums.length)];
				const track = album.tracks[Math.floor(rand() * album.tracks.length)];
				const length = durationOf(current.artist, track);
				if (at > now - 15 * 60 * 1000) break;
				listens.push(listen(current.artist, track, album, length, at, src));
				at += length + rand() * 20_000;
			}
		}
	}
	// A spelling that the catalog suggests merging (shown on /merges).
	if (user === 'fiona') {
		const album = CATALOG[0].albums[1];
		for (let i = 0; i < 4; i++) {
			const at = midnight - (900 + i * 9) * DAY + 20 * 60 * 60 * 1000;
			listens.push(
				listen('Die Aerzte', 'Junge', { ...album, cover: false }, 190_000, at, SOURCES[0].info)
			);
		}
		listens.sort((a, b) => a.listened_at - b.listened_at);
	}
	return listens;
}

function listen(artist, track, album, length, at, info) {
	const additional_info = { ...info, duration_ms: length };
	if (album.cover !== false)
		additional_info.release_group_mbid = fakeMbid(`${artist}/${album.title}`);
	return {
		listened_at: Math.floor(at / 1000),
		track_metadata: {
			artist_name: artist,
			track_name: track,
			release_name: album.title,
			additional_info
		}
	};
}
