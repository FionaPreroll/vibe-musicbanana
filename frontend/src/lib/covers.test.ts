// Album covers: the helpers, the Cover component, and where pages show them.

import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { coverUrl, coversShown, initial, useCovers } from '#lib/covers.svelte.ts';
import Chart from '#lib/components/Chart.svelte';
import Cover from '#lib/components/Cover.svelte';
import { fakeApi } from '../test/fetch.ts';
import * as fixtures from '../test/fixtures.ts';
import { event } from '../test/load.ts';
import { load as layoutLoad } from '../routes/+layout.ts';
import ProfilePage from '../routes/u/[username]/[[slug]]/+page.svelte';
import EntityPage from '../routes/u/[username]/[[slug]]/[kind=entity]/[id=id]/+page.svelte';
import SearchPage from '../routes/u/[username]/[[slug]]/search/+page.svelte';

vi.mock('$app/state', () => import('../test/app.svelte.ts'));
vi.mock('$app/navigation', () => import('../test/app.svelte.ts'));

const api = '/api/profiles/fiona/default';

afterEach(() => {
	useCovers(false);
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe('cover helpers', () => {
	it('builds the cover addresses under a profile', () => {
		expect(coverUrl(api, 10)).toBe(`${api}/releases/10/cover`);
		expect(coverUrl(api, 10, 500)).toBe(`${api}/releases/10/cover?size=500`);
	});

	it('takes the first letter or digit for the placeholder', () => {
		expect(initial('Ärzte')).toBe('Ä');
		expect(initial('  ...und jetzt?')).toBe('U');
		expect(initial('1984')).toBe('1');
		expect(initial('!!!')).toBe('?');
		expect(initial('')).toBe('?');
	});

	it('shows covers only once the server says so', () => {
		expect(coversShown()).toBe(false);
		useCovers(true);
		expect(coversShown()).toBe(true);
	});
});

describe('layout', () => {
	it('asks the server whether to show covers', async () => {
		await layoutLoad(
			event({ fetch: fakeApi({ 'GET /api/me': fixtures.me, 'GET /api/config': { covers: true } }) })
		);
		expect(coversShown()).toBe(true);
	});

	it('shows none when the server does not answer', async () => {
		useCovers(true);
		await layoutLoad(event({ fetch: fakeApi({ 'GET /api/me': fixtures.me }) }));
		expect(coversShown()).toBe(false);
	});
});

describe('Cover', () => {
	it('shows the picture, and the letter when it fails to load', async () => {
		render(Cover, { src: '/cover', name: 'Geräusch', alt: 'Cover of Geräusch' });
		const img = screen.getByRole('img', { name: 'Cover of Geräusch' });
		expect(img.tagName).toBe('IMG');
		expect(img).toHaveAttribute('src', '/cover');
		expect(img).toHaveAttribute('loading', 'lazy');
		await fireEvent.error(img);
		const tile = screen.getByRole('img', { name: 'Cover of Geräusch' });
		expect(tile.tagName).toBe('SPAN');
		expect(tile).toHaveTextContent('G');
	});

	it('shows the letter right away without a picture, hidden next to the title', () => {
		const { container } = render(Cover, { src: null, name: 'Debut' });
		expect(screen.queryByRole('img')).toBeNull();
		const tile = container.querySelector('span')!;
		expect(tile).toHaveTextContent('D');
		expect(tile).toHaveAttribute('aria-hidden', 'true');
	});

	it('tries again for another album in the same place', async () => {
		const { rerender } = render(Cover, { src: '/a', name: 'A', alt: 'A' });
		await fireEvent.error(screen.getByRole('img'));
		expect(screen.getByRole('img').tagName).toBe('SPAN');
		await rerender({ src: '/b', name: 'B', alt: 'B' });
		expect(screen.getByRole('img')).toHaveAttribute('src', '/b');
	});
});

describe('Chart with covers', () => {
	it('puts a cover before each entry', () => {
		const { container } = render(Chart, {
			title: 'Top albums',
			entries: fixtures.releases,
			cover: (entry) => coverUrl(api, entry.id)
		});
		expect(container.querySelector('img')).toHaveAttribute('src', `${api}/releases/10/cover`);
	});

	it('shows none without', () => {
		const { container } = render(Chart, { title: 'Top albums', entries: fixtures.releases });
		expect(container.querySelector('img')).toBeNull();
	});
});

describe('pages with covers', () => {
	beforeEach(() => {
		vi.stubGlobal(
			'EventSource',
			class {
				close() {}
				addEventListener() {}
			}
		);
		vi.stubGlobal('fetch', fakeApi({}));
	});

	const profile = () =>
		({
			me: null,
			follow: null,
			api,
			base: '/u/fiona',
			period: { kind: 'all' },
			query: {},
			source: null,
			sources: fixtures.sources,
			overview: fixtures.overview,
			artists: fixtures.artists,
			releases: fixtures.releases,
			recordings: fixtures.recordings,
			clock: Promise.resolve(null),
			lost: Promise.resolve(null),
			recent: {
				listens: [
					fixtures.listen('2026-10-07T09:55:00Z'),
					fixtures.listen('2026-10-06T20:00:00Z', { album: null, release_id: null })
				],
				next: null
			},
			nowPlaying: null,
			week: null,
			onThisDay: null
		}) as never;

	it('shows none on the profile while covers are off', () => {
		const { container } = render(ProfilePage, { params: {} as never, data: profile() });
		expect(container.querySelector('img')).toBeNull();
	});

	it('shows them in the top albums and the recent listens', () => {
		useCovers(true);
		const { container } = render(ProfilePage, { params: {} as never, data: profile() });
		const covers = [...container.querySelectorAll('img')].map((img) => img.getAttribute('src'));
		// Top albums and the first listen; the listen without an album gets a tile.
		expect(covers).toEqual([`${api}/releases/10/cover`, `${api}/releases/10/cover`]);
		const listens = screen.getByRole('heading', { name: /listens/i }).parentElement!;
		expect(within(listens).getAllByText('S', { selector: 'span' }).length).toBe(1);
	});

	it('shows a large cover on the album page', () => {
		useCovers(true);
		render(EntityPage, {
			params: {} as never,
			data: {
				me: null,
				base: '/u/fiona',
				api,
				kind: 'album',
				source: null,
				sources: [],
				entity: {
					...fixtures.entity,
					id: 10,
					name: 'Die Bestie in Menschengestalt',
					artist: { id: 1, name: 'Die Ärzte' }
				}
			} as never
		});
		const cover = screen.getByRole('img', { name: 'Cover of Die Bestie in Menschengestalt' });
		expect(cover).toHaveAttribute('src', `${api}/releases/10/cover`);
		expect(cover).toHaveAttribute(
			'srcset',
			`${api}/releases/10/cover 1x, ${api}/releases/10/cover?size=500 2x`
		);
	});

	it('shows them for the albums found', () => {
		useCovers(true);
		const { container } = render(SearchPage, {
			params: {} as never,
			data: {
				me: null,
				base: '/u/fiona',
				api,
				overview: fixtures.overview,
				q: 'bestie',
				source: null,
				sources: [],
				found: { artists: [], releases: fixtures.releases, recordings: [] }
			} as never
		});
		expect(container.querySelector('img')).toHaveAttribute('src', `${api}/releases/10/cover`);
	});
});
