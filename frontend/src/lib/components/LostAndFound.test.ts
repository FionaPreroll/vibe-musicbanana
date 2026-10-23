import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LostAndFound as Lost, LostEntry } from '#lib/api.ts';
import { fakeApi } from '../../test/fetch.ts';
import LostAndFound from './LostAndFound.svelte';

const entry = (id: number, name: string, artist?: string): LostEntry => ({
	id,
	name,
	artist,
	listens: 100 + id,
	first_listened_at: '2009-05-01T12:00:00Z',
	last_listened_at: id % 2 ? '2012-05-01T12:00:00Z' : '2009-06-01T12:00:00Z'
});

const lost = (fields: Partial<Lost> = {}): Lost => ({
	since: '2025-10-01T00:00:00Z',
	artists: [entry(1, 'Die Ärzte'), entry(2, 'Björk')],
	tracks: [entry(11, 'Westerland', 'Die Ärzte'), entry(12, 'Jóga', 'Björk')],
	hidden: 2,
	...fields
});

const api = '/api/profiles/fiona/default';
const hiddenApi = '/api/me/profiles/default/lost-and-found/hidden';
const href = (kind: string, id: number) => `/${kind}/${id}`;

beforeEach(() => {
	vi.useFakeTimers({ now: new Date('2026-10-07T10:00:00Z'), toFake: ['Date'] });
});
afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('LostAndFound', () => {
	it('lists what was heard a lot, for anybody', () => {
		render(LostAndFound, { lost: lost(), api, own: null, source: null, href });
		expect(screen.getByText('Heard a lot, but not since Oct 1, 2025')).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'Westerland' })).toHaveAttribute('href', '/track/11');
		expect(screen.getByRole('link', { name: 'Björk' })).toHaveAttribute('href', '/artist/2');
		expect(
			screen.getByText('101 listens · 2009–2012 · last heard 14 years ago')
		).toBeInTheDocument();
		expect(screen.getByText('102 listens · 2009 · last heard 17 years ago')).toBeInTheDocument();
		expect(screen.queryByRole('button')).toBeNull();
	});

	it('shows five at first', async () => {
		const artists = Array.from({ length: 7 }, (_, i) => entry(i + 1, `Artist ${i + 1}`));
		render(LostAndFound, {
			lost: lost({ artists, tracks: [], since: null }),
			api,
			own: null,
			source: null,
			href
		});
		expect(screen.getByText('Heard a lot, but not since a long time')).toBeInTheDocument();
		expect(screen.getByText('Nothing lost right now.')).toBeInTheDocument();
		expect(screen.queryByText('Artist 6')).toBeNull();
		await userEvent.click(screen.getByRole('button', { name: 'More' }));
		expect(screen.getByText('Artist 7')).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Fewer' }));
		expect(screen.queryByText('Artist 7')).toBeNull();
	});

	it('lets the owner dismiss an artist and take it back', async () => {
		const posted: unknown[] = [];
		const fetch = fakeApi({
			[`POST ${hiddenApi}`]: (init: RequestInit) => {
				posted.push(JSON.parse(String(init.body)));
				return null;
			},
			[`DELETE ${hiddenApi}/artist/1`]: null,
			[`GET ${api}/lost-and-found`]: (_: RequestInit, url: URL) => {
				expect(url.search).toBe('?limit=10&source=Navidrome');
				return lost({
					artists: [entry(2, 'Björk'), entry(3, 'Tocotronic')],
					tracks: [entry(12, 'Jóga', 'Björk')]
				});
			}
		});
		vi.stubGlobal('fetch', fetch);
		render(LostAndFound, { lost: lost(), api, own: 'default', source: 'Navidrome', href });
		await userEvent.click(screen.getByRole('button', { name: 'Dismiss Die Ärzte' }));
		expect(posted).toEqual([{ kind: 'artist', id: 1 }]);
		expect(screen.getByText('Dismissed Die Ärzte and its tracks.')).toBeInTheDocument();
		expect(await screen.findByRole('link', { name: 'Tocotronic' })).toBeInTheDocument();
		expect(screen.queryByRole('link', { name: 'Westerland' })).toBeNull();
		await userEvent.click(screen.getByRole('button', { name: 'Undo' }));
		expect(fetch).toHaveBeenCalledWith(`${hiddenApi}/artist/1`, expect.anything());
		expect(screen.queryByText(/Dismissed Die Ärzte/)).toBeNull();
	});

	it('dismisses a track', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({
				[`POST ${hiddenApi}`]: null,
				[`GET ${api}/lost-and-found`]: lost({ tracks: [entry(12, 'Jóga', 'Björk')] })
			})
		);
		render(LostAndFound, { lost: lost(), api, own: 'default', source: null, href });
		await userEvent.click(screen.getByRole('button', { name: 'Dismiss Westerland' }));
		expect(screen.getByText('Dismissed Westerland.')).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'Die Ärzte' })).toBeInTheDocument();
	});

	it('shows what was dismissed and brings it back', async () => {
		let hidden = [
			{ kind: 'artist', id: 7, name: 'Tocotronic', hidden_at: '2026-01-02T12:00:00Z' },
			{
				kind: 'recording',
				id: 8,
				name: 'Westerland',
				artist: 'Die Ärzte',
				hidden_at: '2026-01-03T12:00:00Z'
			}
		];
		vi.stubGlobal(
			'fetch',
			fakeApi({
				[`GET ${hiddenApi}`]: () => hidden,
				[`DELETE ${hiddenApi}/recording/8`]: () => {
					hidden = hidden.slice(0, 1);
					return null;
				},
				[`GET ${api}/lost-and-found`]: lost({ hidden: 1 })
			})
		);
		render(LostAndFound, { lost: lost(), api, own: 'default', source: null, href });
		const toggle = screen.getByRole('button', { name: 'Dismissed (2)' });
		await userEvent.click(toggle);
		expect(toggle).toHaveAttribute('aria-expanded', 'true');
		expect(await screen.findByText('· artist and its tracks')).toBeInTheDocument();
		expect(screen.getByText('Jan 3, 2026')).toBeInTheDocument();
		const again = screen.getAllByRole('button', { name: 'Show again' });
		await userEvent.click(again[1]);
		await vi.waitFor(() =>
			expect(screen.getAllByRole('button', { name: 'Show again' })).toHaveLength(1)
		);
		await userEvent.click(screen.getByRole('button', { name: 'Hide dismissed' }));
		expect(screen.queryByText('Show again')).toBeNull();
	});

	it('says when nothing is dismissed, and shows errors', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({
				[`GET ${hiddenApi}`]: [],
				[`POST ${hiddenApi}`]: new Response('database is down', { status: 500 }),
				[`GET ${api}/lost-and-found`]: new Response('', { status: 500 })
			})
		);
		render(LostAndFound, { lost: lost(), api, own: 'default', source: null, href });
		await userEvent.click(screen.getByRole('button', { name: 'Dismissed (2)' }));
		expect(await screen.findByText('Nothing dismissed.')).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Dismiss Björk' }));
		expect(await screen.findByText('Database is down')).toBeInTheDocument();
	});

	it('shows an error when the dismissed ones do not load', async () => {
		vi.stubGlobal('fetch', fakeApi({}));
		render(LostAndFound, { lost: lost(), api, own: 'default', source: null, href });
		await userEvent.click(screen.getByRole('button', { name: 'Dismissed (2)' }));
		expect(await screen.findByText(/^No route GET/)).toBeInTheDocument();
	});
});
