// Comparing two profiles: the helpers, the shared chart, both pages and their loads.

import { render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Comparison, ProfileSummary } from '#lib/api.ts';
import SharedChart from '#lib/components/SharedChart.svelte';
import {
	combinedYears,
	compareApi,
	comparePath,
	earliest,
	latest,
	scoreLabel
} from '#lib/compare.ts';
import type { Period } from '#lib/period.ts';
import { visit } from '../test/app.svelte.ts';
import { fakeApi } from '../test/fetch.ts';
import * as fixtures from '../test/fixtures.ts';
import { event, thrown } from '../test/load.ts';
import PickPage from './u/[username]/[[slug]]/compare/+page.svelte';
import { load as pickLoad } from './u/[username]/[[slug]]/compare/+page.ts';
import ComparePage from './u/[username]/[[slug]]/compare/[other]/[[otherSlug]]/+page.svelte';
import { load as compareLoad } from './u/[username]/[[slug]]/compare/[other]/[[otherSlug]]/+page.ts';

vi.mock('$app/state', () => import('../test/app.svelte.ts'));
vi.mock('$app/navigation', () => import('../test/app.svelte.ts'));

const comparison: Comparison = {
	a: {
		username: 'fiona',
		slug: 'default',
		name: 'Default',
		listens: 2048,
		artists: 120,
		only_count: 12,
		only: [{ id: 3, name: 'Björk', listens: 40 }]
	},
	b: {
		username: 'alex',
		slug: 'work',
		name: 'Work',
		listens: 512,
		artists: 30,
		only_count: 1,
		only: [{ id: 4, name: 'Tiësto', listens: 7 }]
	},
	score: 0.42,
	shared: { artists: 1, releases: 2, recordings: 1 },
	artists: [{ id: 1, name: 'Die Ärzte', a: 300, b: 25 }],
	releases: [
		{ id: 10, name: 'Geräusch', artist: 'Die Ärzte', a: 40, b: 5 },
		{ id: 11, name: 'Jazz ist anders', artist: 'Die Ärzte', a: 20, b: 10 }
	],
	recordings: []
};

const alexOverview = {
	...fixtures.overview,
	username: 'alex',
	slug: 'work',
	name: 'Work',
	own: false,
	first_listened_at: '2005-01-01T12:00:00Z',
	last_listened_at: '2026-10-08T12:00:00Z',
	years: [
		{ year: 2005, listens: 10 },
		{ year: 2010, listens: 100 }
	]
};

beforeEach(() => {
	vi.useFakeTimers({ now: new Date('2026-10-07T10:00:00Z'), toFake: ['Date'] });
	Element.prototype.scrollIntoView = vi.fn();
});
afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe('compare helpers', () => {
	it('builds paths', () => {
		expect(comparePath('/u/fiona', 'alex')).toBe('/u/fiona/compare/alex');
		expect(comparePath('/u/fiona/work', 'Die Ärzte', 'tour 2026')).toBe(
			'/u/fiona/work/compare/Die%20%C3%84rzte/tour%202026'
		);
		expect(compareApi(['fiona', 'default'], ['alex', 'a/b'])).toBe(
			'/api/compare/fiona/default/alex/a%2Fb'
		);
	});

	it('puts the overlap in words', () => {
		expect(scoreLabel(0.8)).toBe('Musical twins');
		expect(scoreLabel(0.5)).toBe('Musical twins');
		expect(scoreLabel(0.3)).toBe('Very similar taste');
		expect(scoreLabel(0.2)).toBe('Similar taste');
		expect(scoreLabel(0.05)).toBe('Some common ground');
		expect(scoreLabel(0.001)).toBe('Little in common');
		expect(scoreLabel(0)).toBe('Nothing in common');
	});

	it('adds up the years and keeps the gaps', () => {
		expect(
			combinedYears(
				[
					{ year: 2007, listens: 3 },
					{ year: 2009, listens: 1 }
				],
				[{ year: 2007, listens: 2 }]
			)
		).toEqual([
			{ year: 2007, listens: 5 },
			{ year: 2008, listens: 0 },
			{ year: 2009, listens: 1 }
		]);
		expect(combinedYears([], [])).toEqual([]);
		// No century of empty years.
		expect(combinedYears([{ year: 1, listens: 1 }], [{ year: 2007, listens: 1 }])).toEqual([
			{ year: 1, listens: 1 },
			{ year: 2007, listens: 1 }
		]);
	});

	it('picks the earliest and latest instant', () => {
		const [old, recent] = ['2007-08-12T14:28:00Z', '2026-10-07T09:55:00Z'];
		expect(earliest(recent, old)).toBe(old);
		expect(earliest(old, recent)).toBe(old);
		expect(earliest(null, recent)).toBe(recent);
		expect(earliest(old, null)).toBe(old);
		expect(latest(old, recent)).toBe(recent);
		expect(latest(recent, old)).toBe(recent);
		expect(latest(null, null)).toBeNull();
	});
});

describe('shared chart', () => {
	it('shows both counts with a bar for each', () => {
		render(SharedChart, {
			title: 'Albums',
			entries: comparison.releases,
			names: ['fiona', 'alex'],
			href: (e) => `/album/${e.id}`
		});
		const rows = screen.getAllByRole('listitem');
		expect(rows).toHaveLength(2);
		expect(within(rows[0]).getByRole('link')).toHaveAttribute('href', '/album/10');
		expect(rows[0]).toHaveTextContent('Geräusch · Die Ärzte');
		expect(within(rows[0]).getByTitle('fiona')).toHaveTextContent('40');
		expect(within(rows[0]).getByTitle('alex')).toHaveTextContent('5');
	});

	it('says when there is nothing in common', () => {
		render(SharedChart, { title: 'Tracks', entries: [], names: ['fiona', 'alex'] });
		expect(screen.getByText('Nothing in common.')).toBeInTheDocument();
	});
});

describe('compare page', () => {
	const data = (fields: Record<string, unknown> = {}) =>
		({
			me: null,
			period: { kind: 'all' } as Period,
			source: null,
			comparison,
			overviews: [fixtures.overview, alexOverview],
			bases: ['/u/fiona', '/u/alex/work'],
			...fields
		}) as never;

	it('shows the overlap, what both hear and what only one hears', () => {
		visit('/u/fiona/compare/alex/work');
		render(ComparePage, { params: {} as never, data: data() });

		expect(screen.getByText('42%')).toBeInTheDocument();
		expect(screen.getByText('Very similar taste')).toBeInTheDocument();
		expect(screen.getByText(/same artists · All time/)).toBeInTheDocument();
		expect(screen.getByText(/1 artist · 2 albums · 1 track in common/)).toBeInTheDocument();
		for (const link of screen.getAllByRole('link', { name: 'fiona' })) {
			expect(link).toHaveAttribute('href', '/u/fiona');
		}
		expect(screen.getByRole('link', { name: 'alex / Work' })).toHaveAttribute(
			'href',
			'/u/alex/work'
		);
		expect(screen.getByRole('link', { name: /Swap/ })).toHaveAttribute(
			'href',
			'/u/alex/work/compare/fiona'
		);
		expect(screen.getAllByRole('link', { name: /Die Ärzte/ })[0]).toHaveAttribute(
			'href',
			'/u/fiona/artist/1-die-arzte'
		);
		expect(screen.getByText('Only fiona')).toBeInTheDocument();
		expect(screen.getByRole('link', { name: /Tiësto/ })).toHaveAttribute(
			'href',
			'/u/alex/work/artist/4-tiesto'
		);
		// Fiona has more such artists than shown, alex none.
		expect(screen.getByText('12 artists in all')).toBeInTheDocument();
		expect(screen.queryByText('1 artists in all')).not.toBeInTheDocument();
		// The year bar spans both profiles.
		expect(screen.getByRole('link', { name: '2005' })).toBeInTheDocument();
		expect(screen.getByRole('link', { name: '2026' })).toBeInTheDocument();
	});

	it('keeps the period and source in its links', () => {
		visit('/u/fiona/compare/alex/work?year=2010&source=Navidrome');
		render(ComparePage, {
			params: {} as never,
			data: data({
				period: { kind: 'year', year: 2010 },
				source: 'Navidrome',
				comparison: {
					...comparison,
					score: 1 / 3,
					shared: { artists: 2, releases: 1, recordings: 3 },
					a: { ...comparison.a, artists: 1 },
					b: { ...comparison.b, only_count: 5 }
				}
			})
		});
		expect(screen.getByText(/same artists · 2010/)).toBeInTheDocument();
		expect(screen.getByText(/2 artists · 1 album · 3 tracks in common/)).toBeInTheDocument();
		expect(screen.getByText(/1 artist$/)).toBeInTheDocument();
		expect(screen.getByText('5 artists in all')).toBeInTheDocument();
		expect(screen.getByRole('link', { name: /Swap/ })).toHaveAttribute(
			'href',
			'/u/alex/work/compare/fiona?year=2010&source=Navidrome'
		);
		expect(screen.getAllByRole('link', { name: /Die Ärzte/ })[0]).toHaveAttribute(
			'href',
			'/u/fiona/artist/1-die-arzte?source=Navidrome'
		);
	});
});

describe('compare picker', () => {
	const profiles: ProfileSummary[] = [
		{ username: 'fiona', slug: 'default', name: 'Default', visibility: 'public', listens: 2048 },
		{ username: 'fiona', slug: 'work', name: 'Work', visibility: 'private', listens: 10 },
		{ username: 'alex', slug: 'default', name: 'Default', visibility: 'followers', listens: 5 },
		{ username: 'empty', slug: 'default', name: 'Default', visibility: 'public', listens: 0 }
	];

	it('offers the viewer’s profiles first, without this one and empty ones', async () => {
		const fetch = fakeApi({
			'GET /api/profiles/alex/default': { ...alexOverview, slug: 'default', name: 'Default' },
			'GET /api/profiles': profiles
		});
		const data = await pickLoad(
			event({ fetch, me: fixtures.me, path: '/u/alex/compare', params: { username: 'alex' } })
		);
		expect(data).toMatchObject({
			base: '/u/alex',
			own: [{ slug: 'default' }, { slug: 'work' }],
			others: []
		});

		render(PickPage, { params: {} as never, data: { me: fixtures.me, ...data } as never });
		expect(screen.getByText('Your profiles')).toBeInTheDocument();
		expect(screen.getByRole('link', { name: /Work/ })).toHaveAttribute(
			'href',
			'/u/alex/compare/fiona/work'
		);
	});

	it('lists everybody else to a guest', async () => {
		const fetch = fakeApi({
			'GET /api/profiles/fiona/default': fixtures.overview,
			'GET /api/profiles': profiles
		});
		const data = await pickLoad(
			event({ fetch, path: '/u/fiona/compare', params: { username: 'fiona' } })
		);
		expect(data).toMatchObject({
			own: [],
			others: [{ username: 'fiona', slug: 'work' }, { username: 'alex' }]
		});

		render(PickPage, { params: {} as never, data: { me: null, ...data } as never });
		expect(screen.getByText('Profiles')).toBeInTheDocument();
		expect(screen.getByRole('link', { name: /alex/ })).toHaveAttribute(
			'href',
			'/u/fiona/compare/alex'
		);
	});

	it('says when there is nobody to compare with', () => {
		render(PickPage, {
			params: {} as never,
			data: {
				me: null,
				base: '/u/fiona',
				overview: fixtures.overview,
				own: [],
				others: []
			} as never
		});
		expect(screen.getByText(/no other profile with listens/)).toBeInTheDocument();
	});

	it('answers 404 for an unknown profile', async () => {
		const fetch = fakeApi({ 'GET /api/profiles': profiles });
		const error = await thrown(
			pickLoad(event({ fetch, path: '/u/nobody/compare', params: { username: 'nobody' } }))
		);
		expect(error).toMatchObject({ status: 404 });
	});

	it('passes other errors on', async () => {
		const fetch = fakeApi({
			'GET /api/profiles/fiona/default': () => new Response('down', { status: 500 }),
			'GET /api/profiles': profiles
		});
		const error = await thrown(
			pickLoad(event({ fetch, path: '/u/fiona/compare', params: { username: 'fiona' } }))
		);
		expect(error).toMatchObject({ status: 500 });
	});
});

describe('compare load', () => {
	const routes = (fields: Record<string, unknown> = {}) => ({
		'GET /api/compare/fiona/default/alex/work': (_: unknown, url: URL) => ({
			...comparison,
			search: url.search
		}),
		'GET /api/profiles/fiona/default': fixtures.overview,
		'GET /api/profiles/alex/work': alexOverview,
		...fields
	});
	const params = { username: 'fiona', other: 'alex', otherSlug: 'work' };

	it('loads the comparison for the period', async () => {
		const data = await compareLoad(
			event({
				fetch: fakeApi(routes()),
				path: '/u/fiona/compare/alex/work?year=2010&source=Navidrome',
				params
			})
		);
		expect(data).toMatchObject({
			period: { kind: 'year', year: 2010 },
			source: 'Navidrome',
			comparison: { search: '?year=2010&tz=Europe%2FBerlin&limit=10&source=Navidrome' },
			overviews: [{ username: 'fiona' }, { username: 'alex' }],
			bases: ['/u/fiona', '/u/alex/work']
		});
	});

	it('refuses a bad period', async () => {
		const error = await thrown(
			compareLoad(event({ fetch: fakeApi(routes()), path: '/x?days=0', params }))
		);
		expect(error).toMatchObject({ status: 400 });
	});

	it('passes errors of the API on', async () => {
		for (const [status, text] of [
			[404, 'There is no such profile.'],
			[400, 'a profile cannot be compared with itself'],
			[500, 'down']
		] as const) {
			const fetch = fakeApi(
				routes({ 'GET /api/profiles/alex/work': () => new Response(text, { status }) })
			);
			const error = await thrown(compareLoad(event({ fetch, path: '/x', params })));
			expect(error).toMatchObject({ status });
		}
	});
});
