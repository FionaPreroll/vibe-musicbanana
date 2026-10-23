// The load functions of all pages, against a fake API.

import { afterEach, describe, expect, it } from 'vitest';
import { useTimeSettings, timeZone } from '#lib/zone.svelte.ts';
import { fakeApi } from '../test/fetch.ts';
import * as fixtures from '../test/fixtures.ts';
import { event, thrown } from '../test/load.ts';
import { load as layoutLoad } from './+layout.ts';
import { load as homeLoad } from './+page.ts';
import { load as mergesLoad } from './merges/+page.ts';
import { load as settingsLoad } from './settings/+page.ts';
import { load as historyLoad } from './settings/history/[slug]/+page.ts';
import { load as statusLoad } from './status/+page.ts';
import { load as profileLoad } from './u/[username]/[[slug]]/+page.ts';
import { load as entityLoad } from './u/[username]/[[slug]]/[kind=entity]/[id=id]/+page.ts';
import { load as followLoad } from './u/[username]/[[slug]]/follow/+page.ts';
import { load as searchLoad } from './u/[username]/[[slug]]/search/+page.ts';
import { load as yearLoad } from './u/[username]/[[slug]]/year/[year=year]/+page.ts';

const { me } = fixtures;
const fail =
	(status: number, text = '') =>
	() =>
		new Response(text, { status });

/** The profile page's data, with its promises for what comes later. */
type ProfileData = { clock: Promise<unknown>; lost: Promise<unknown> };

afterEach(() => useTimeSettings(null));

describe('layout', () => {
	it('loads who is logged in and takes their time settings', async () => {
		const fetch = fakeApi({ 'GET /api/me': { ...me, time_zone: 'Asia/Tokyo' } });
		const data = await layoutLoad(event({ fetch }));
		expect(data).toMatchObject({ me: { username: 'fiona' } });
		expect(timeZone()).toBe('Asia/Tokyo');
	});

	it('knows nobody without a session', async () => {
		const fetch = fakeApi({ 'GET /api/me': fail(401) });
		expect(await layoutLoad(event({ fetch }))).toEqual({ me: null });
	});

	it('passes other errors on', async () => {
		const fetch = fakeApi({ 'GET /api/me': fail(500, 'database is down') });
		await expect(layoutLoad(event({ fetch }))).rejects.toThrow('database is down');
	});
});

describe('start page', () => {
	it('loads the profiles, and whom the viewer follows', async () => {
		const fetch = fakeApi({ 'GET /api/profiles': [], 'GET /api/me/following': [1] });
		expect(await homeLoad(event({ fetch, me }))).toEqual({ profiles: [], following: [1] });
		expect(await homeLoad(event({ fetch }))).toEqual({ profiles: [], following: [] });
	});
});

describe('admin pages', () => {
	it('send guests to the login', async () => {
		const fetch = fakeApi({});
		expect(await thrown(statusLoad(event({ fetch })))).toMatchObject({
			status: 307,
			location: '/login?next=/status'
		});
		expect(await thrown(mergesLoad(event({ fetch, path: '/merges?kind=release' })))).toMatchObject({
			location: '/login?next=%2Fmerges%3Fkind%3Drelease'
		});
	});

	it('tell others they are for admins', async () => {
		const fetch = fakeApi({
			'GET /api/admin/status': fail(403),
			'GET /api/admin/merges/suggestions/artist': fail(403),
			'GET /api/admin/merges': fail(403)
		});
		expect(await thrown(statusLoad(event({ fetch, me })))).toMatchObject({
			status: 403,
			body: { message: 'The status page is for admins.' }
		});
		expect(await thrown(mergesLoad(event({ fetch, me })))).toMatchObject({
			status: 403,
			body: { message: 'Merges are for admins.' }
		});
	});

	it('pass other errors on', async () => {
		const fetch = fakeApi({});
		await expect(statusLoad(event({ fetch, me }))).rejects.toThrow(/no route/);
		await expect(mergesLoad(event({ fetch, me }))).rejects.toThrow(/no route/);
	});

	it('load the status', async () => {
		const fetch = fakeApi({ 'GET /api/admin/status': { build: {} } });
		expect(await statusLoad(event({ fetch, me }))).toEqual({ status: { build: {} } });
	});

	it('load the suggestions of the kind asked for', async () => {
		const fetch = fakeApi({
			'GET /api/admin/merges/suggestions/release': { total: 0 },
			'GET /api/admin/merges/suggestions/artist': { total: 1 },
			'GET /api/admin/merges': { merges: [] }
		});
		expect(await mergesLoad(event({ fetch, me, path: '/merges?kind=release' }))).toEqual({
			kind: 'release',
			suggestions: { total: 0 },
			log: { merges: [] }
		});
		expect(await mergesLoad(event({ fetch, me, path: '/merges?kind=nonsense' }))).toMatchObject({
			kind: 'artist'
		});
	});
});

describe('settings', () => {
	it('sends guests to the login', async () => {
		expect(await thrown(settingsLoad(event({ fetch: fakeApi({}) })))).toMatchObject({
			location: '/login?next=/settings'
		});
	});

	it('loads everything of the account', async () => {
		const fetch = fakeApi({
			'GET /api/me/profiles': ['profiles'],
			'GET /api/me/tokens': ['tokens'],
			'GET /api/me/yourspotify': ['connections'],
			'GET /api/me/yourspotify/allowed': ['allowed'],
			'GET /api/me/followers': ['followers']
		});
		expect(await settingsLoad(event({ fetch, me }))).toEqual({
			me,
			profiles: ['profiles'],
			tokens: ['tokens'],
			connections: ['connections'],
			allowed: ['allowed'],
			followers: ['followers']
		});
	});
});

describe('listening history', () => {
	const api = '/api/me/profiles/default';
	const routes = {
		[`GET ${api}/history`]: (_: unknown, url: URL) => ({ search: url.search }),
		[`GET ${api}/trash`]: { total: 2 },
		'GET /api/profiles/fiona/default/sources': []
	};

	it('sends guests to the login and refuses foreign profiles', async () => {
		const fetch = fakeApi(routes);
		const path = '/settings/history/default?q=x';
		expect(
			await thrown(historyLoad(event({ fetch, path, params: { slug: 'default' } })))
		).toMatchObject({ location: '/login?next=%2Fsettings%2Fhistory%2Fdefault%3Fq%3Dx' });
		expect(
			await thrown(historyLoad(event({ fetch, me, path, params: { slug: 'nope' } })))
		).toMatchObject({ status: 404 });
	});

	it('loads the filtered history and the trash', async () => {
		const fetch = fakeApi(routes);
		const data = await historyLoad(
			event({
				fetch,
				me,
				path: '/settings/history/default?q=%C3%A4rzte&from=2026-10-05T20:00&source=Navidrome',
				params: { slug: 'default' }
			})
		);
		expect(data).toMatchObject({
			api,
			trash: false,
			filter: { q: 'ärzte', from: '2026-10-05T20:00', to: '', source: 'Navidrome' },
			history: { search: '?q=%C3%A4rzte&from=2026-10-05T18%3A00%3A00.000Z&source=Navidrome' },
			inTrash: { total: 2 }
		});
	});

	it('skips the history in the trash', async () => {
		const fetch = fakeApi(routes);
		const data = await historyLoad(
			event({ fetch, me, path: '/settings/history/default?trash', params: { slug: 'default' } })
		);
		expect(data).toMatchObject({ trash: true, history: null });
	});

	it('turns a bad filter into an error page', async () => {
		const fetch = fakeApi({ ...routes, [`GET ${api}/history`]: fail(400, 'bad time') });
		const path = '/settings/history/default';
		expect(
			await thrown(historyLoad(event({ fetch, me, path, params: { slug: 'default' } })))
		).toMatchObject({ status: 400, body: { message: 'bad time' } });
		const broken = fakeApi({ ...routes, [`GET ${api}/trash`]: fail(500) });
		expect(
			await thrown(historyLoad(event({ fetch: broken, me, path, params: { slug: 'default' } })))
		).toMatchObject({ status: 500 });
	});
});

describe('profile page', () => {
	const api = '/api/profiles/fiona/default';
	const routes = (fields: Record<string, unknown> = {}) => ({
		[`GET ${api}`]: fixtures.overview,
		[`GET ${api}/top/artists`]: (_: unknown, url: URL) => ({ search: url.search }),
		[`GET ${api}/top/releases`]: [],
		[`GET ${api}/top/recordings`]: [],
		[`GET ${api}/clock`]: { listens: 0, weekdays: [] },
		[`GET ${api}/lost-and-found`]: fail(500),
		[`GET ${api}/listens`]: (_: unknown, url: URL) => ({ search: url.search }),
		[`GET ${api}/now-playing`]: null,
		[`GET ${api}/sources`]: [],
		[`GET ${api}/week`]: { listens: 1 },
		[`GET ${api}/on-this-day`]: fail(500),
		[`GET ${api}/follow`]: { state: 'none' },
		...fields
	});
	const params = { username: 'fiona' };

	it('loads all time with the week and the day', async () => {
		const fetch = fakeApi(routes());
		const data = (await profileLoad(event({ fetch, path: '/u/fiona', params }))) as ProfileData;
		expect(data).toMatchObject({
			api,
			base: '/u/fiona',
			period: { kind: 'all' },
			source: null,
			follow: null,
			artists: { search: '?tz=Europe%2FBerlin&limit=10' },
			recent: { search: '?limit=25' },
			nowPlaying: null,
			week: { listens: 1 },
			onThisDay: null
		});
		await expect(data.clock).resolves.toEqual({ listens: 0, weekdays: [] });
		await expect(data.lost).resolves.toBeNull();
	});

	it('loads a year from a source, and the follow state of somebody else', async () => {
		const fetch = fakeApi(routes({ [`GET ${api}`]: { ...fixtures.overview, own: false } }));
		const data = (await profileLoad(
			event({ fetch, me, path: '/u/fiona?year=2012&source=Navidrome', params })
		)) as ProfileData;
		expect(data).toMatchObject({
			period: { kind: 'year', year: 2012 },
			source: 'Navidrome',
			follow: { state: 'none' },
			artists: { search: '?year=2012&tz=Europe%2FBerlin&limit=10&source=Navidrome' },
			recent: { search: '?before=2012-12-31T23%3A00%3A00.000Z&limit=25&source=Navidrome' },
			week: null,
			onThisDay: null
		});
		await expect(data.lost).resolves.toBeNull();
	});

	it('refuses a malformed period', async () => {
		const fetch = fakeApi(routes());
		expect(
			await thrown(profileLoad(event({ fetch, path: '/u/fiona?days=0', params })))
		).toMatchObject({ status: 400 });
	});

	it('follows a renamed account', async () => {
		const fetch = fakeApi({ 'GET /api/renamed/fiona/default': { username: 'fi' } });
		expect(
			await thrown(profileLoad(event({ fetch, path: '/u/fiona?year=2012', params })))
		).toMatchObject({ status: 308, location: '/u/fi?year=2012' });
	});

	it('leads to the door of a profile for followers', async () => {
		const fetch = fakeApi({
			[`GET ${api}`]: fail(404),
			[`GET ${api}/follow`]: { username: 'fiona', slug: 'default' }
		});
		expect(await thrown(profileLoad(event({ fetch, me, path: '/u/fiona', params })))).toMatchObject(
			{
				status: 307,
				location: '/u/fiona/follow'
			}
		);
	});

	it('says when there is no such profile', async () => {
		const fetch = fakeApi({ [`GET ${api}`]: fail(404) });
		expect(await thrown(profileLoad(event({ fetch, me, path: '/u/fiona', params })))).toMatchObject(
			{
				status: 404,
				body: { message: 'There is no such profile.' }
			}
		);
		expect(await thrown(profileLoad(event({ fetch, path: '/u/fiona', params })))).toMatchObject({
			status: 404
		});
	});

	it('shows what the API says is wrong', async () => {
		const fetch = fakeApi(routes({ [`GET ${api}`]: fail(400, 'unknown time zone') }));
		expect(await thrown(profileLoad(event({ fetch, path: '/u/fiona', params })))).toMatchObject({
			status: 400,
			body: { message: 'unknown time zone' }
		});
		const down = fakeApi(routes({ [`GET ${api}`]: fail(503) }));
		expect(
			await thrown(profileLoad(event({ fetch: down, path: '/u/fiona', params })))
		).toMatchObject({ status: 503 });
	});
});

describe('follow page', () => {
	const params = { username: 'bela', slug: 'work' };
	it('loads the follow state', async () => {
		const fetch = fakeApi({ 'GET /api/profiles/bela/work/follow': { state: 'requested' } });
		expect(await followLoad(event({ fetch, me, params }))).toEqual({
			info: { state: 'requested' }
		});
		expect(
			await thrown(followLoad(event({ fetch, path: '/u/bela/work/follow', params })))
		).toMatchObject({ location: '/login?next=%2Fu%2Fbela%2Fwork%2Ffollow' });
	});

	it('says when there is no such profile', async () => {
		const fetch = fakeApi({ 'GET /api/profiles/bela/default/follow': fail(404) });
		expect(
			await thrown(followLoad(event({ fetch, me, params: { username: 'bela' } })))
		).toMatchObject({ status: 404 });
		const down = fakeApi({ 'GET /api/profiles/bela/default/follow': fail(500) });
		expect(
			await thrown(followLoad(event({ fetch: down, me, params: { username: 'bela' } })))
		).toMatchObject({ status: 500 });
	});
});

describe('search page', () => {
	const api = '/api/profiles/fiona/default';
	const routes = {
		[`GET ${api}`]: fixtures.overview,
		[`GET ${api}/search`]: (_: unknown, url: URL) => ({ search: url.search }),
		[`GET ${api}/sources`]: []
	};
	const params = { username: 'fiona' };

	it('searches when there is something to search', async () => {
		const fetch = fakeApi(routes);
		expect(
			await searchLoad(event({ fetch, path: '/u/fiona/search?q=+%C3%A4rzte+', params }))
		).toMatchObject({ base: '/u/fiona', q: 'ärzte', found: { search: '?q=%C3%A4rzte&limit=20' } });
		expect(await searchLoad(event({ fetch, path: '/u/fiona/search', params }))).toMatchObject({
			q: '',
			found: { artists: [], releases: [], recordings: [] }
		});
	});

	it('follows renames and says when there is no such profile', async () => {
		const fetch = fakeApi({ [`GET ${api}`]: fail(404) });
		expect(
			await thrown(searchLoad(event({ fetch, path: '/u/fiona/search', params })))
		).toMatchObject({ status: 404 });
		const down = fakeApi({ ...routes, [`GET ${api}`]: fail(500) });
		expect(
			await thrown(searchLoad(event({ fetch: down, path: '/u/fiona/search', params })))
		).toMatchObject({ status: 500 });
	});
});

describe('artist, album and track pages', () => {
	const api = '/api/profiles/fiona/default';
	const routes = {
		[`GET ${api}/artists/1`]: fixtures.entity,
		[`GET ${api}/sources`]: []
	};
	const params = { username: 'fiona', kind: 'artist', id: 1 };

	it('loads the entry at its current address', async () => {
		const fetch = fakeApi(routes);
		const data = await entityLoad(event({ fetch, path: '/u/fiona/artist/1-die-arzte', params }));
		expect(data).toMatchObject({ base: '/u/fiona', kind: 'artist', entity: { name: 'Die Ärzte' } });
	});

	it('moves old addresses to the current one', async () => {
		const fetch = fakeApi(routes);
		expect(
			await thrown(entityLoad(event({ fetch, path: '/u/fiona/artist/1?source=x', params })))
		).toMatchObject({ status: 308, location: '/u/fiona/artist/1-die-arzte?source=x' });
	});

	it('says what is missing or wrong', async () => {
		const fetch = fakeApi({});
		expect(
			await thrown(entityLoad(event({ fetch, path: '/u/fiona/artist/1', params })))
		).toMatchObject({ status: 404, body: { message: 'There is no such profile or artist.' } });
		const bad = fakeApi({ ...routes, [`GET ${api}/artists/1`]: fail(400, 'bad zone') });
		expect(
			await thrown(entityLoad(event({ fetch: bad, path: '/u/fiona/artist/1', params })))
		).toMatchObject({ status: 400 });
		const down = fakeApi({ ...routes, [`GET ${api}/artists/1`]: fail(500) });
		expect(
			await thrown(entityLoad(event({ fetch: down, path: '/u/fiona/artist/1', params })))
		).toMatchObject({ status: 500 });
	});
});

describe('year in review', () => {
	const api = '/api/profiles/fiona/work';
	const routes = {
		[`GET ${api}`]: fixtures.overview,
		[`GET ${api}/year/2010`]: fixtures.review,
		[`GET ${api}/top/artists`]: (_: unknown, url: URL) => ({ search: url.search }),
		[`GET ${api}/top/releases`]: [],
		[`GET ${api}/top/recordings`]: [],
		[`GET ${api}/sources`]: []
	};
	const params = { username: 'fiona', slug: 'work', year: 2010 };

	it('loads the review and the charts of the year', async () => {
		const fetch = fakeApi(routes);
		expect(
			await yearLoad(event({ fetch, path: '/u/fiona/work/year/2010?source=Navidrome', params }))
		).toMatchObject({
			base: '/u/fiona/work',
			source: 'Navidrome',
			review: { year: 2010 },
			artists: { search: '?year=2010&tz=Europe%2FBerlin&limit=10&source=Navidrome' }
		});
	});

	it('says what is missing or wrong', async () => {
		const path = '/u/fiona/work/year/2010';
		expect(await thrown(yearLoad(event({ fetch: fakeApi({}), path, params })))).toMatchObject({
			status: 404
		});
		const bad = fakeApi({ ...routes, [`GET ${api}/year/2010`]: fail(400, 'too early') });
		expect(await thrown(yearLoad(event({ fetch: bad, path, params })))).toMatchObject({
			status: 400,
			body: { message: 'too early' }
		});
		const down = fakeApi({ ...routes, [`GET ${api}/year/2010`]: fail(500) });
		expect(await thrown(yearLoad(event({ fetch: down, path, params })))).toMatchObject({
			status: 500
		});
	});
});
