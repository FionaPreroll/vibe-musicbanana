// Sample answers of the API for page tests, after a real profile: Fiona, who has
// heard Die Ärzte a lot since 2007.

import type { EntityPage, Listen, Me, Overview, Review, Source } from '#lib/api.ts';

export const me: Me = {
	username: 'fiona',
	email: 'fiona@example.org',
	admin: false,
	time_zone: null,
	week_start: 1,
	show_streaks: true,
	language: null,
	profiles: [
		{ slug: 'default', name: 'Default', visibility: 'public' },
		{ slug: 'work', name: 'Work', visibility: 'private' }
	]
};

export const admin: Me = { ...me, admin: true };

export const overview: Overview = {
	username: 'fiona',
	slug: 'default',
	name: 'Default',
	visibility: 'public',
	own: true,
	listens: 2048,
	first_listened_at: '2007-08-12T14:28:00Z',
	last_listened_at: '2026-10-07T09:55:00Z',
	years: [
		{ year: 2007, listens: 300 },
		{ year: 2010, listens: 900 },
		{ year: 2026, listens: 848 }
	]
};

export const sources: Source[] = [
	{
		source: 'Navidrome',
		listens: 1000,
		first_listened_at: '2020-01-01T12:00:00Z',
		last_listened_at: '2026-10-07T09:55:00Z'
	},
	{
		source: 'import:php',
		listens: 1048,
		first_listened_at: '2007-08-12T14:28:00Z',
		last_listened_at: '2016-01-01T12:00:00Z'
	}
];

export const artists = [
	{ id: 1, name: 'Die Ärzte', listens: 800 },
	{ id: 2, name: 'Björk', listens: 300 }
];
export const releases = [
	{ id: 10, name: 'Die Bestie in Menschengestalt', artist: 'Die Ärzte', listens: 200 }
];
export const recordings = [
	{ id: 20, name: 'Schrei nach Liebe', artist: 'Die Ärzte', listens: 90 },
	{ id: 21, name: 'Jóga', artist: 'Björk', listens: 40 }
];

export const listen = (listened_at: string, fields: Partial<Listen> = {}): Listen => ({
	listened_at,
	artist: 'Die Ärzte',
	track: 'Schrei nach Liebe',
	album: 'Die Bestie in Menschengestalt',
	artist_id: 1,
	recording_id: 20,
	release_id: 10,
	...fields
});

export const entity: EntityPage = {
	profile: { username: 'fiona', slug: 'default', name: 'Default' },
	id: 1,
	name: 'Die Ärzte',
	mbids: ['f2fb0ff0-5679-42ec-a55c-15109ce6e320'],
	listens: 800,
	first_listened_at: '2007-08-12T14:28:00Z',
	last_listened_at: '2026-10-07T09:55:00Z',
	months: [
		{ month: '2007-08', listens: 300 },
		{ month: '2007-09', listens: 500 }
	],
	phases: [{ from: '2007-08', to: '2007-09', listens: 800 }],
	releases,
	recordings
};

export const review: Review = {
	year: 2010,
	complete: true,
	listens: 900,
	last_year_so_far: 600,
	last_year: 600,
	artists: 120,
	recordings: 400,
	days: 200,
	new_artists: 30,
	discoveries: [
		{ id: 2, name: 'Björk', listens: 50, first_listened_at: '2010-03-27T17:05:00Z' },
		{ id: 3, name: 'Tocotronic', listens: 20, first_listened_at: '2010-06-01T10:00:00Z' }
	],
	risers: [
		{ id: 1, name: 'Die Ärzte', listens: 300, last_year: 100, rank: 1, last_rank: 4 },
		{ id: 4, name: 'Element of Crime', listens: 80, last_year: 0, rank: 5, last_rank: null },
		{ id: 5, name: 'Kraftklub', listens: 60, last_year: 50, rank: 8, last_rank: 7 }
	],
	longest_session: {
		started_at: '2010-07-01T18:00:00Z',
		ended_at: '2010-07-01T21:56:00Z',
		listens: 61,
		artists: [
			{ id: 1, name: 'Die Ärzte', listens: 40 },
			{ id: 2, name: 'Björk', listens: 15 },
			{ id: 3, name: 'Tocotronic', listens: 6 }
		]
	},
	months: Array.from({ length: 12 }, (_, i) => ({
		month: i + 1,
		listens: i === 6 ? 200 : 60,
		last_year: 50
	})),
	top_day: { date: '2010-07-01', listens: 61 }
};
