// The English texts. This catalog sets the shape every other language follows
// (see Messages); a text with values is a function of them. Templates with
// {name} holes are filled with markup by <Fill> (see Fill.svelte).

import { formatNumber as n, plural } from './intl.ts';

const listens = (count: number) =>
	`${n(count)} ${plural(count, { one: 'listen', other: 'listens' })}`;
const plays = (count: number) => `${n(count)} ${plural(count, { one: 'play', other: 'plays' })}`;
const days = (count: number) => `${n(count)} ${plural(count, { one: 'day', other: 'days' })}`;

const en = {
	/** The language's own name, for the language picker. */
	languageName: 'English',

	listens,
	plays,
	days,
	/** A stretch of time in hours and minutes. */
	duration: (hours: number, minutes: number) =>
		hours === 0
			? `${minutes} min`
			: minutes === 0
				? `${n(hours)} h`
				: `${n(hours)} h ${minutes} min`,
	/** Units of `timeSince` ("19 years, 1 month"), largest first. */
	since: {
		units: [
			(c: number) => `${c} ${plural(c, { one: 'year', other: 'years' })}`,
			(c: number) => `${c} ${plural(c, { one: 'month', other: 'months' })}`,
			(c: number) => `${c} ${plural(c, { one: 'week', other: 'weeks' })}`,
			(c: number) => `${c} ${plural(c, { one: 'day', other: 'days' })}`,
			(c: number) => `${c} ${plural(c, { one: 'hour', other: 'hours' })}`,
			(c: number) => `${c} ${plural(c, { one: 'minute', other: 'minutes' })}`
		],
		two: (first: string, second: string) => `${first}, ${second}`
	},

	common: {
		loading: 'Loading…',
		showMore: 'Show more',
		show: 'Show',
		undo: 'Undo',
		cancel: 'Cancel',
		save: 'Save',
		saved: 'Saved.',
		remove: 'Remove',
		none: 'None.',
		from: 'From',
		to: 'To',
		fewer: 'Fewer',
		more: 'More',
		artists: 'Artists',
		albums: 'Albums',
		tracks: 'Tracks',
		topArtists: 'Top artists',
		topAlbums: 'Top albums',
		topTracks: 'Top tracks',
		/** Two days or times as the ends of a stretch. */
		/** A name in quotation marks. */
		quote: (text: string) => `“${text}”`,
		dateRange: (from: string, to: string) => `${from} to ${to}`
	},

	nav: {
		search: (username: string) => `Search ${username}`,
		searchLabel: 'Search the artists, albums and tracks of this profile',
		searchKey: 'Press / to search',
		menu: 'Menu',
		status: 'Status',
		merges: 'Merges',
		settings: 'Settings',
		logOut: 'Log out',
		logIn: 'Log in',
		darkMode: 'Dark mode',
		toLight: 'Switch to light mode',
		toDark: 'Switch to dark mode'
	},

	errors: {
		backHome: 'Back to the start page',
		noProfile: 'There is no such profile.',
		noEntity: {
			artist: 'There is no such profile or artist.',
			album: 'There is no such profile or album.',
			track: 'There is no such profile or track.'
		},
		noOwnProfile: 'You have no such profile.',
		invalidPeriod: 'Invalid period',
		mergesForAdmins: 'Merges are for admins.',
		statusForAdmins: 'The status page is for admins.'
	},

	home: {
		tagline: 'A free place for your listening history.',
		youFollow: 'You follow',
		asked: 'Asked, no answer yet',
		allProfiles: 'All profiles',
		profiles: 'Profiles',
		noProfiles: 'No profiles to show yet.'
	},

	login: {
		title: 'Log in',
		login: 'User name or email',
		password: 'Password',
		busy: 'Logging in…',
		submit: 'Log in',
		help: 'The password of the old musicbanana still works. A new account comes from {create} on the server, and a forgotten password is replaced there with {reset}.',
		usernameArg: '<username>'
	},

	visibility: {
		public: 'Public',
		followers: 'Followers only',
		private: 'Private',
		privateTitle: 'Only you see this profile',
		followersTitle: 'You and your followers see this profile'
	},

	follow: {
		ask: 'Ask to follow',
		follow: 'Follow',
		asked: (username: string) => `Asked ${username} to follow`,
		takeBack: 'Take back',
		following: 'Following',
		unfollow: 'Unfollow',
		confirmStop: (username: string) =>
			`Stop following? You won't see this profile until ${username} says yes again.`,
		goToProfile: 'Go to the profile',
		followersOnly: (username: string) =>
			`${username} shows this profile to followers only. It opens for you once ${username} says yes to your request.`
	},

	source: {
		label: 'Source',
		from: 'From',
		all: 'All sources',
		unknown: 'Unknown player',
		oldMusicbanana: 'Old musicbanana'
	},

	banana: {
		lives: (since: string) => `musicbanana already lives ${since}`,
		python: (since: string) => `has been a happy python for ${since}`,
		elephpant: (since: string) => `has been a happy ElePHPant for ${since}`,
		rustacean: (since: string) => `has been a happy Rustacean for ${since}`,
		sinceTitle: (when: string) => `since ${when}`
	},

	chart: {
		nothing: 'Nothing here.',
		/** The badge for an entry that was not in the chart of the period before. */
		new: 'new',
		newTitle: 'Not heard in the period before',
		same: '=',
		sameTitle: 'Same place as in the period before',
		upTitle: (places: number, previous: number) =>
			`Up ${n(places)} ${plural(places, { one: 'place', other: 'places' })}, from ${n(previous)}`,
		downTitle: (places: number, previous: number) =>
			`Down ${n(places)} ${plural(places, { one: 'place', other: 'places' })}, from ${n(previous)}`,
		showMore: 'Show more',
		showLess: 'Show less',
		loading: 'Loading…',
		failed: 'Could not load more.',
		/** Below the charts: what their arrows compare with. */
		comparedWithYear: (year: number) => `Arrows compare each place with ${year}.`,
		comparedWithDays: (days: number) =>
			days === 1
				? 'Arrows compare each place with the day before.'
				: `Arrows compare each place with the ${n(days)} days before.`
	},

	onThisDay: {
		heading: 'On this day',
		sub: (day: string) => `${day} in earlier years`,
		yearsAgo: (years: number) => (years === 1 ? 'A year ago' : `${n(years)} years ago`),
		fewerYears: 'Fewer years',
		allYears: (years: number) => `All ${n(years)} years`
	},

	catalogPicker: {
		pickAnother: 'Pick another',
		placeholder: 'Name or #id'
	},

	clock: {
		heading: 'Listening clock',
		sub: (zone: string) => `By weekday and hour, in ${zone}`,
		busiestDay: 'Busiest day',
		busiestHour: 'Busiest hour',
		share: (percent: string) => `${percent} of the listens`,
		label: (day: string, hours: string) =>
			`Listens by weekday and hour; most on ${day} and at ${hours}`,
		fewer: 'Fewer',
		more: 'More listens'
	},

	period: {
		label: 'Period',
		allTime: 'All time',
		today: 'Today',
		lastDays: (count: number) => `Last ${n(count)} days`,
		since: (day: string) => `Since ${day}`,
		until: (day: string) => `Until ${day}`,
		custom: 'Custom'
	},

	monthCurve: {
		label: (from: string, to: string) => `Listens per month from ${from} to ${to}`,
		gap: (after: string, before: string) => `No listens between ${after} and ${before}`
	},

	week: {
		heading: 'This week',
		listens: 'Listens',
		compare: (diff: number, lastWeek: number, partial: boolean) => {
			const by = partial ? ' by now' : '';
			if (lastWeek === 0) return `none last week${by}`;
			if (diff === 0) return `as many as last week${by}`;
			return `${n(Math.abs(diff))} ${diff > 0 ? 'more' : 'fewer'} than last week${by}`;
		},
		daysWithListens: 'Days with listens',
		of: (total: number) => `of ${n(total)}`,
		streak: 'Streak',
		inARowSince: (day: string) => `in a row since ${day}`,
		noneYesterday: 'no listens yesterday or today',
		longestStreak: 'Longest streak',
		on: (day: string) => `on ${day}`,
		fromTo: (from: string, to: string) => `from ${from} to ${to}`,
		thisWeek: 'This week',
		lastWeek: 'Last week',
		stillToCome: 'still to come',
		artists: 'Artists of the week',
		firstTime: (count: number) => `${n(count)} heard for the first time`,
		noneYet: 'No listens yet this week.',
		new: 'new',
		lastWeekCount: (count: number) => `${n(count)} last week`
	},

	lost: {
		heading: 'Lost & found',
		notSince: (day: string | null) => `Heard a lot, but not since ${day ?? 'a long time'}`,
		nothing: 'Nothing lost right now.',
		lastHeardOn: (day: string) => `Last heard ${day}`,
		lastHeard: (ago: string) => `last heard ${ago}`,
		notInterested: 'Not interested any more',
		dismiss: (name: string) => `Dismiss ${name}`,
		dismissed: (name: string, artist: boolean) =>
			`Dismissed ${name}${artist ? ' and its tracks' : ''}.`,
		hideDismissed: 'Hide dismissed',
		dismissedCount: (count: number) => `Dismissed (${n(count)})`,
		artistAndTracks: 'artist and its tracks',
		showAgain: 'Show again',
		nothingDismissed: 'Nothing dismissed.'
	},

	artistYears: {
		heading: 'Top artists over the years',
		sub: 'The most heard artists of each year. A line follows an artist from one year to the next.',
		longest: 'Longest at the top:',
		region: 'Top artists of each year',
		gap: (after: number, before: number) =>
			after === before ? `No listens in ${after}` : `No listens from ${after} to ${before}`,
		chartsOf: (year: number) => `Charts of ${year}`,
		topOf: (year: number) => `Top artists of ${year}`,
		place: (name: string, rank: number, year: number, listens: string) =>
			`${name}, number ${rank} in ${year}, ${listens}`,
		tip: (rank: number, year: number) => `#${rank} in ${year}`
	},

	profile: {
		span: (from: string, to: string) => `from ${from} to ${to}`,
		nowPlaying: 'Now playing',
		inReview: (year: number) => `${year} in review →`,
		lastOfYear: (year: number) => `Last listens of ${year}`,
		lastUpTo: (day: string) => `Last listens up to ${day}`,
		recent: 'Recent listens',
		noListens: 'No listens.',
		older: 'Older listens'
	},

	entity: {
		kinds: { artist: 'Artist', album: 'Album', track: 'Track' },
		musicBrainzOf: (i: number, count: number) => `MusicBrainz ${i} of ${count}`,
		coverOf: (album: string) => `Cover of ${album}`,
		/** {artist} is a link. */
		by: 'by {artist}',
		listens: 'Listens',
		firstListen: 'First listen',
		lastListen: 'Last listen',
		perMonth: 'Listens per month',
		notHeard: 'Not heard in this profile.',
		heavy: 'Heavy listening',
		onAlbums: 'On albums'
	},

	search: {
		title: 'Search',
		placeholder: 'Artist, album or track',
		label: (username: string) => `Search the artists, albums and tracks of ${username}`,
		nothing: (q: string) => `Nothing heard matches “${q}”.`,
		hint: 'Every word counts, in any case and with or without accents; “ärzte unrockbar” finds the track.'
	},

	review: {
		title: (year: number) => `${year} in review`,
		crumb: 'Year in review',
		soFar: 'so far',
		otherYears: 'Other years',
		noListens: (year: number, partial: boolean) =>
			`No listens in ${year}${partial ? ' by now' : ''}.`,
		compare: (
			change: number,
			lastYear: number,
			year: number,
			partial: boolean,
			percent: string
		) => {
			const by = partial ? ' by now' : '';
			if (lastYear === 0) return `none in ${year}${by}`;
			if (change === 0) return `as many as in ${year}${by}`;
			return `${percent} ${change > 0 ? 'more' : 'fewer'} than in ${year}${by}`;
		},
		newAndTracks: (fresh: number, tracks: number) =>
			`${n(fresh)} new, ${n(tracks)} ${plural(tracks, { one: 'track', other: 'tracks' })}`,
		daysWithListens: 'Days with listens',
		busiestMonth: 'Busiest month',
		busiestDay: 'Busiest day',
		longestSession: 'Longest session',
		listensOn: (listens: string, day: string) => `${listens} on ${day}`,
		discovered: (year: number) => `Discovered in ${year}`,
		noDiscoveries: 'No artists heard for the first time.',
		discoveries: (count: number, more: boolean) =>
			`${n(count)} ${plural(count, { one: 'artist', other: 'artists' })} heard for the first time${more ? ', the most heard of them' : ''}`,
		since: (day: string) => `since ${day}`,
		risers: 'Biggest risers',
		noRisers: (year: number, partial: boolean) =>
			`No artist heard before got more listens than in ${year}${partial ? ' by now' : ''}.`,
		risersSub: (year: number, partial: boolean) =>
			`Artists heard before, with the most listens more than in ${year}${partial ? ' by now' : ''}`,
		back: 'back',
		notHeardIn: (year: number, partial: boolean) =>
			`Not heard in ${year}${partial ? ' by now' : ''}`,
		places: (before: number, year: number, partial: boolean) =>
			`Place in the artist charts of ${before}${partial ? ' by now' : ''} and of ${year}`,
		/** {artists} is the list of links. */
		mostly:
			'Mostly {artists}. A session goes on while the next listen starts at most 30 minutes after the end of the one before.'
	},

	settings: {
		title: 'Settings',
		profiles: {
			heading: 'Profiles',
			intro:
				'Each profile has its own listens and charts, for example one per player or per person.',
			editHistory: 'Edit listening history',
			whoSees: (name: string) => `Who sees ${name}`,
			nameOf: (slug: string) => `Name of ${slug}`,
			rename: 'Rename',
			new: 'New profile',
			address: 'Address',
			addressRule: 'Lower-case letters, digits and dashes',
			name: 'Name',
			whoSeesIt: 'Who sees it',
			addressMakes: (path: string) => `The address makes ${path}.`,
			create: 'Create profile'
		},
		tokens: {
			heading: 'Scrobble tokens',
			intro:
				'A player scrobbles with a token, which decides the profile its listens go to. In Navidrome it goes into the personal settings under "Scrobble to ListenBrainz".',
			/** {label} is the token's label, in bold. */
			created: 'The token for {label}. It is shown only now, so copy it into the player:',
			done: 'Done',
			none: 'No tokens yet.',
			label: 'Label',
			profile: 'Profile',
			lastUsed: 'Last used',
			never: 'never',
			revoke: 'Revoke',
			confirmRevoke: (label: string) =>
				`Revoke the token "${label}"? Its client can't scrobble any more.`,
			create: 'Create token'
		},
		yourspotify: {
			heading: 'Spotify via YourSpotify',
			/** {yourspotify} is a link, {fetchAll} the name of the button. */
			intro:
				'{yourspotify} keeps the history of a Spotify account. Connected to a profile, musicbanana imports its whole history, then the new plays every 15 minutes. One Spotify account per profile. When YourSpotify has imported an older Spotify export since, {fetchAll} brings its plays; those already here are skipped.',
			soFar: (listens: string) => `${listens} so far`,
			fetchAll: 'Fetch all again',
			fetchAllTitle: 'Fetch the whole history again, for plays YourSpotify has added from before',
			confirmDisconnect: (profile: string) =>
				`Stop importing from YourSpotify into ${profile}? Its listens stay.`,
			confirmRefetch: (profile: string) =>
				`Fetch the whole history from YourSpotify into ${profile} again? Plays it has already are skipped.`,
			/** {variable} is the name of the setting. */
			cliOnly:
				'This server takes YourSpotify connections from its command line only. Its admin can allow addresses for this page with {variable}.',
			profile: 'Profile',
			address: 'YourSpotify API address',
			token: 'Public token',
			checking: 'Checking…',
			connect: 'Connect',
			addressHelp: (addresses: string) =>
				`The address is YourSpotify's API (its API_ENDPOINT, not the web interface), as the musicbanana server reaches it: ${addresses}, or below. The public token is in YourSpotify's settings.`
		},
		followers: {
			heading: 'Followers',
			intro:
				'Anybody logged in can follow a public profile. A profile for followers shows itself to those you said yes to; removing someone hides it from them again.',
			none: 'Nobody follows your profiles yet.',
			/** {user} and {profile} are links. */
			asks: (since: string) => `{user} asks to follow {profile} since ${since}`,
			follows: (since: string) => `{user} follows {profile} as of ${since}`,
			yes: 'Say yes',
			no: 'Say no',
			confirmNo: (username: string) => `Say no to ${username}?`,
			confirmRemove: (username: string) => `Remove ${username}?`
		},
		time: {
			heading: 'Time',
			intro:
				"Days, weeks, years and the listening clock follow your time zone, on your profiles and on everybody else's. Without one, they follow this browser's.",
			zone: 'Time zone',
			browserZone: (zone: string) => `This browser's (${zone})`,
			weekStart: 'Weeks start on'
		},
		language: {
			heading: 'Language',
			intro: "The language of these pages. Without a choice, they follow this browser's.",
			browser: (name: string) => `This browser's (${name})`
		},
		streaks: {
			heading: 'Streaks',
			text: 'Show streaks, the days in a row with listens, under "This week" on profile pages. Turned off, you don\'t see them on any profile, and nobody sees those of your profiles.'
		},
		username: {
			heading: 'User name',
			intro:
				'It is in the address of your profiles and logs you in, as does your email address. Links with an old name lead to the new one, and nobody else can take it. Scrobble tokens and YourSpotify connections stay as they are.',
			label: 'User name',
			rename: 'Rename',
			/** {link} is the new address. */
			renamed: (old: string) => `Renamed. Links with ${old} lead to {link}.`
		},
		email: {
			heading: 'Email address',
			intro:
				'It logs you in, as does your user name. musicbanana sends no mail, so it is not checked.',
			new: 'New address',
			password: 'Your password',
			changed: (address: string) => `Changed to ${address}.`,
			change: 'Change address'
		},
		password: {
			heading: 'Password',
			current: 'Current password',
			new: 'New password',
			again: 'New password again',
			differ: 'The new passwords differ.',
			changed: 'Changed. Other browsers logged in with this account are logged out.',
			change: 'Change password'
		},
		data: {
			heading: 'Your data',
			intro:
				"One JSON file with everything musicbanana keeps about your account: the account, your profiles with all their listens (in ListenBrainz's import format) and their trash, follows, scrobble tokens, YourSpotify connections and logins. Passwords and tokens stay out.",
			download: 'Download your data'
		},
		delete: {
			heading: 'Deleting a profile or the account',
			text: "Ask the admin of this server. A deleted profile takes its listens, scrobble tokens, YourSpotify connection and followers along; a deleted account all its profiles and logins. This can't be undone."
		}
	},

	connection: {
		starts: 'Starts within a minute.',
		importing: (since: string) => `Importing since ${since}…`,
		failed: (at: string, error: string) => `Failed at ${at}: ${error}`,
		upToDate: (at: string) => `Up to date as of ${at}.`,
		refetching: (found: number, added: number, upTo: string | null) =>
			`Fetching everything again: ${plays(found)} so far, ${n(added)} of them new${upTo ? `, up to ${upTo}` : ''}…`,
		refetchStopped: (added: number, upTo: string | null) =>
			`Fetching everything again stopped${upTo ? `, up to ${upTo}` : ''} with ${n(added)} new so far; it goes on in the next round.`,
		refetchAfterImport: 'Fetches everything again once this import is done.',
		refetchSoon: 'Fetches everything again within a minute.',
		refetched: (at: string, found: number, added: number, there: number, leftOut: number) =>
			`Fetched everything again on ${at}: ${plays(found)}, ${n(added)} new, ${n(there)} already here` +
			(leftOut > 0 ? `, ${n(leftOut)} left out as YourSpotify knows none of their artists` : '') +
			'.'
	},

	history: {
		title: 'Edit listening history',
		/** {profile} is a link. */
		intro:
			"of {profile}. Listens in the trash leave the charts and every page, and stay there until you restore them or empty the trash. A player or an import sending them again doesn't bring them back.",
		listens: 'Listens',
		trash: 'Trash',
		words: 'Words',
		placeholder: 'Artist, track or album',
		source: 'Source',
		clear: 'Clear',
		inTrash: (count: number) => `${listens(count)} in the trash.`,
		total: (count: number, filtered: boolean) =>
			filtered
				? `${listens(count)} ${plural(count, { one: 'matches', other: 'match' })}.`
				: `${listens(count)}.`,
		selected: (count: number) => `${n(count)} selected.`,
		restoreSelected: 'Restore selected',
		restoreAll: 'Restore all',
		deleteSelected: 'Delete selected for good',
		emptyTrash: 'Empty the trash',
		moveSelected: 'Move selected to the trash',
		moveAll: (count: number) => `Move all ${n(count)} to the trash`,
		moved: (count: number) => `Moved ${listens(count)} to the trash.`,
		confirmMoveAll: (count: number) => `Move all ${listens(count)} that match to the trash?`,
		restored: (restored: number, kept: number) =>
			`Restored ${listens(restored)}.` +
			(kept
				? ` ${listens(kept)} stayed in the trash: the profile has another listen at that time now.`
				: ''),
		confirmDelete: (count: number) => `Delete ${listens(count)} for good? This can't be undone.`,
		deleted: (count: number) => `Deleted ${listens(count)} for good.`,
		trashEmpty: 'The trash is empty.',
		noListens: 'No listens.',
		selectAll: 'Select all shown',
		select: (artist: string, track: string, at: string) => `Select ${artist} – ${track}, ${at}`,
		time: 'Time',
		listen: 'Listen',
		trashed: 'Trashed'
	},

	merges: {
		title: 'Merges',
		intro:
			'Merging puts two entries of the catalog together that are the same under different spellings, for every profile. The listens keep what the players sent, later listens with either spelling count for the one that stays, and every merge can be undone in the log below.',
		kind: { artist: 'artist', release: 'album', recording: 'track' },
		kinds: { artist: 'Artists', release: 'Albums', recording: 'Tracks' },
		likeness: {
			same_letters: [
				'Same letters',
				'Only case, accents, punctuation, spaces, a leading "The" or "&" for "and" differ.'
			],
			one_letter: ['One letter apart', 'One letter more, missing, different or swapped.'],
			version: [
				'Another version?',
				'The same title apart from a note such as "(Live)", "- Radio Edit" or "feat. …". Live versions and the original are better kept apart.'
			]
		} as Record<'same_letters' | 'one_letter' | 'version', [string, string]>,
		merged: (kind: string, from: string, into: string, count: number) =>
			`Merged ${kind} “${from}” into “${into}”: ${listens(count)} moved over.`,
		hidden: 'Hidden. It stays out of the suggestions until you show it again.',
		suggestedAgain: (a: string, b: string) => `“${a}” and “${b}” are suggested again.`,
		undid: (id: number, kind: string, from: string) =>
			`Undid merge ${id}: ${kind} “${from}” stands on its own again.`,
		suggestions: 'Suggestions',
		noneLeft: 'No look-alikes left.',
		firstOf: (count: number, total: number) =>
			`The first ${n(count)} of ${n(total)}, the most certain first.`,
		count: (count: number) =>
			`${n(count)} ${plural(count, { one: 'look-alike', other: 'look-alikes' })}, the most certain first.`,
		withinArtist:
			'Albums and tracks are only compared within an artist, so merging look-alike artists first also merges their albums and tracks of the same title.',
		closeHidden: 'Close the hidden ones',
		hiddenCount: (count: number) => `${n(count)} hidden`,
		pair: (a: string, aId: number, b: string, bId: number) =>
			`“${a}” (#${aId}) and “${b}” (#${bId})`,
		hiddenAt: (at: string) => `hidden ${at}`,
		suggestAgain: 'Suggest again',
		into: 'into',
		mergeEllipsis: 'Merge…',
		otherWay: 'Other way…',
		otherWayTitle: (from: string, into: string) =>
			`Keep “${from}” and merge “${into}” into it instead`,
		hide: 'Hide',
		hideTitle: "These are different ones; don't suggest them again",
		byHand: 'Merge by hand',
		anyTwo: (kinds: string) =>
			`Any two ${kinds.toLowerCase()}, also ones the suggestions don't find.`,
		fromLabel: 'Merge (goes away)',
		intoLabel: 'Into (stays)',
		swap: 'Swap',
		preview: 'Preview',
		/** {from} and {into} are the names, in bold. */
		wouldMerge: (kind: string) => `Would merge ${kind} {from} into {into}.`,
		countFor: (count: number, spellings: number, into: string) =>
			`${listens(count)} and ${n(spellings)} ${plural(spellings, { one: 'spelling', other: 'spellings' })} count for “${into}” from then on.`,
		albumsMoved: (moved: number, merged: number) =>
			`Albums: ${n(moved)} moved over, ${n(merged)} merged with one of the same title.`,
		tracksMoved: (moved: number, merged: number) =>
			`Tracks: ${n(moved)} moved over, ${n(merged)} merged with one of the same title.`,
		toldApart:
			"Their MusicBrainz IDs say these are different ones. Merge anyway, for example an artist's other name or a track MusicBrainz lists twice.",
		merge: 'Merge',
		log: 'Log',
		/** {editions} is the command. */
		logIntro:
			'All merges, from here, the command line and {editions}, the latest first. Undoing one puts back what it changed; later merges of the same entries have to be undone first.',
		noMerges: 'No merges yet.',
		undone: 'undone',
		undoEllipsis: 'Undo…',
		wouldUndo: (id: number, kind: string, from: string, restored: number, kept: number) =>
			`Would undo merge ${id}: ${kind} “${from}” stands on its own again, with ${n(restored)} ${plural(restored, { one: 'change', other: 'changes' })} put back.` +
			(kept
				? ` ${n(kept)} ${plural(kept, { one: 'row stays', other: 'rows stay' })} as it is now, as it changed again since.`
				: ''),
		undoMerge: 'Undo merge'
	},

	status: {
		title: 'Status',
		refresh: 'Refreshes every 10 seconds.',
		build: 'Build',
		version: 'Version',
		debugBuild: '(debug build)',
		commit: 'Commit',
		unknownCommit: 'not known (local build)',
		started: 'Started',
		uptime: 'Uptime',
		process: 'Process',
		cpu: (percent: string, seconds: string) => `${percent} % of a core (${seconds} s in all)`,
		memory: 'Memory',
		memoryValue: (now: string, peak: string) => `${now}, at most ${peak}`,
		threads: 'Threads',
		machine: 'Machine',
		machineValue: (cores: number, load: string) => `${n(cores)} cores, load ${load}`,
		machineMemory: 'Machine memory',
		free: (free: string, total: string, limit: string | null) =>
			`${free} free of ${total}${limit ? `, limit ${limit}` : ''}`,
		database: 'Database',
		size: 'Size',
		migration: 'Migration',
		connections: 'Connections',
		connectionsValue: (busy: number, idle: number, total: string) =>
			`${n(busy)} busy, ${n(idle)} idle in the pool; ${total} to the database`,
		cacheHits: 'Cache hits',
		lastDay: 'Listens in the last 24 hours',
		yourspotify: 'YourSpotify connections',
		lastRound: (at: string | null) =>
			`The server last looked for due connections ${at ? `at ${at}` : 'not yet'}.`,
		routes: 'API response times since the start',
		routesIntro:
			'By route under /api, the ones that took the most time in all first; median and 95th percentile of the latest 500 requests each.',
		route: 'Route',
		requests: 'Requests',
		mean: 'Mean',
		median: 'Median',
		max: 'Max',
		tables: 'Tables',
		table: 'Table',
		rows: 'Rows (about)',
		/** Days and hours, hours and minutes, or minutes and seconds. */
		uptimeValue: (d: number, h: number, m: number, s: number) =>
			d > 0 ? `${d} d ${h} h` : h > 0 ? `${h} h ${m} min` : `${m} min ${s} s`
	},

	/**
	 * The backend's error messages, by the patterns in server-errors.ts. English
	 * shows the backend's own words; another language without one of them too.
	 */
	serverErrors: {} as Partial<import('./server-errors.ts').ServerErrors>
};

export default en;
export type Messages = typeof en;
