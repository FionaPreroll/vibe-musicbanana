// The profile page and the pages below it.

import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NowPlaying, Week } from '#lib/api.ts';
import type { Period } from '#lib/period.ts';
import { useCovers } from '#lib/covers.svelte.ts';
import { goto, visit } from '../test/app.svelte.ts';
import { FakeEventSource } from '../test/events.ts';
import { fakeApi } from '../test/fetch.ts';
import * as fixtures from '../test/fixtures.ts';
import { layOut } from '../test/resize.ts';
import ProfilePage from './u/[username]/[[slug]]/+page.svelte';
import EntityPage from './u/[username]/[[slug]]/[kind=entity]/[id=id]/+page.svelte';
import FollowPage from './u/[username]/[[slug]]/follow/+page.svelte';
import SearchPage from './u/[username]/[[slug]]/search/+page.svelte';
import YearPage from './u/[username]/[[slug]]/year/[year=year]/+page.svelte';

vi.mock('$app/state', () => import('../test/app.svelte.ts'));
vi.mock('$app/navigation', () => import('../test/app.svelte.ts'));

const api = '/api/profiles/fiona/default';
const { listen } = fixtures;

beforeEach(() => {
	vi.useFakeTimers({ now: new Date('2026-10-07T10:00:00Z'), toFake: ['Date'] });
	Element.prototype.scrollIntoView = vi.fn();
	goto.mockClear();
	FakeEventSource.install();
});
afterEach(() => {
	useCovers(false);
	vi.useRealTimers();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe('profile page', () => {
	const data = (fields: Record<string, unknown> = {}) =>
		({
			me: null,
			follow: null,
			api,
			base: '/u/fiona',
			period: { kind: 'all' } as Period,
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
					listen('2026-10-07T09:55:00Z'),
					listen('2026-10-06T20:00:00Z', {
						album: null,
						release_id: null,
						track: 'Westerland',
						recording_id: 21
					})
				],
				next: null
			},
			nowPlaying: null,
			week: null,
			onThisDay: null,
			...fields
		}) as never;

	const routes = (fields: Record<string, unknown> = {}) =>
		fakeApi({
			[`GET ${api}/top/artists/years`]: [
				{ year: 2007, listens: 300, artists: [fixtures.artists[0]] },
				{ year: 2008, listens: 100, artists: [fixtures.artists[0]] }
			],
			...fields
		});

	it('shows the profile with its charts and recent listens', async () => {
		vi.stubGlobal('fetch', routes());
		layOut(800);
		render(ProfilePage, { params: {} as never, data: data() });
		expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('fiona');
		expect(screen.getByRole('link', { name: 'Compare' })).toHaveAttribute(
			'href',
			'/u/fiona/compare'
		);
		expect(
			screen.getByText(/2,048 listens\s+from Aug 12, 2007 to Oct 7, 2026/)
		).toBeInTheDocument();
		// Years without listens stay as gaps.
		expect(screen.getByRole('link', { name: '2008' })).toHaveAttribute('title', '0 listens');
		expect(screen.getByRole('link', { name: /Die Ärzte\s+800/ })).toHaveAttribute(
			'href',
			'/u/fiona/artist/1-die-arzte'
		);
		expect(screen.getByRole('heading', { name: 'Recent listens' })).toBeInTheDocument();
		expect(screen.getByText('Today')).toBeInTheDocument();
		expect(screen.getByText('Yesterday')).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'Westerland' })).toHaveAttribute(
			'href',
			'/u/fiona/track/21-westerland'
		);
		expect(
			screen.getAllByRole('link', { name: 'Die Bestie in Menschengestalt' })[0]
		).toHaveAttribute('href', '/u/fiona/album/10-die-bestie-in-menschengestalt');
		expect(await screen.findByText('Top artists over the years')).toBeInTheDocument();
		expect(screen.queryByText('Older listens')).toBeNull();
	});

	it('shows the week, this day, the clock and lost & found', async () => {
		vi.stubGlobal('fetch', routes());
		const week: Partial<Week> = {
			from: '2026-10-05',
			to: '2026-10-11',
			day: '2026-10-07',
			days: [],
			listens: 0,
			last_week: 0,
			last_week_so_far: 0,
			artists: [],
			new_artists: 0,
			streak: {
				current: 3,
				current_from: '2026-10-05',
				longest: 3,
				longest_from: null,
				longest_to: null
			}
		};
		render(ProfilePage, {
			params: {} as never,
			data: data({
				source: 'Navidrome',
				week,
				onThisDay: {
					day: '2026-10-07',
					years: [{ date: '2016-10-07', years_ago: 10, listens: 3, artists: [], tracks: [] }]
				},
				clock: Promise.resolve({
					listens: 1,
					weekdays: Array.from({ length: 7 }, (_, d) =>
						Array.from({ length: 24 }, (_, h) => (d === 0 && h === 0 ? 1 : 0))
					)
				}),
				lost: Promise.resolve({
					since: null,
					artists: [],
					tracks: [],
					hidden: 1
				})
			})
		});
		expect(screen.getByRole('heading', { name: 'This week' })).toBeInTheDocument();
		expect(screen.getByRole('link', { name: /10 years ago/ })).toHaveAttribute(
			'href',
			'/u/fiona?from=2016-10-07&to=2016-10-07&source=Navidrome'
		);
		expect(await screen.findByText('Listening clock')).toBeInTheDocument();
		expect(await screen.findByText('Lost & found')).toBeInTheDocument();
	});

	it('leaves out a quiet week and empty extras', async () => {
		vi.stubGlobal(
			'fetch',
			routes({ [`GET ${api}/top/artists/years`]: new Response('', { status: 500 }) })
		);
		render(ProfilePage, {
			params: {} as never,
			data: data({
				week: { listens: 0, last_week: 0, streak: null },
				onThisDay: { day: '2026-10-07', years: [] },
				clock: Promise.resolve({ listens: 0, weekdays: [] }),
				lost: Promise.resolve({ since: null, artists: [], tracks: [] }),
				recent: { listens: [], next: null }
			})
		});
		await vi.waitFor(() => expect(screen.getByText('No listens.')).toBeInTheDocument());
		expect(screen.queryByText('This week')).toBeNull();
		expect(screen.queryByText('On this day')).toBeNull();
		expect(screen.queryByText('Listening clock')).toBeNull();
		expect(screen.queryByText('Lost & found')).toBeNull();
		expect(screen.queryByText('Top artists over the years')).toBeNull();
	});

	it('names the listens of a period and links its review', () => {
		vi.stubGlobal('fetch', routes());
		visit('/u/fiona?year=2010');
		const { unmount } = render(ProfilePage, {
			params: {} as never,
			data: data({ period: { kind: 'year', year: 2010 }, source: 'Navidrome' })
		});
		expect(screen.getByRole('heading', { name: 'Last listens of 2010' })).toBeInTheDocument();
		expect(screen.getByRole('link', { name: '2010 in review →' })).toHaveAttribute(
			'href',
			'/u/fiona/year/2010?source=Navidrome'
		);
		unmount();
		render(ProfilePage, {
			params: {} as never,
			data: data({ period: { kind: 'range', from: null, to: '2009-08-31' } })
		});
		expect(
			screen.getByRole('heading', { name: 'Last listens up to Aug 31, 2009' })
		).toBeInTheDocument();
	});

	it('loads older listens', async () => {
		const fetch = routes({
			[`GET ${api}/listens`]: (_: unknown, url: URL) => {
				expect(url.searchParams.get('before')).toBe('cursor');
				return { listens: [listen('2026-09-01T12:00:00Z', { track: 'Zu spät' })], next: null };
			}
		});
		vi.stubGlobal('fetch', fetch);
		render(ProfilePage, {
			params: {} as never,
			data: data({ recent: { listens: [listen('2026-10-07T09:55:00Z')], next: 'cursor' } })
		});
		await userEvent.click(screen.getByRole('button', { name: 'Older listens' }));
		expect(await screen.findByRole('link', { name: 'Zu spät' })).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Older listens' })).toBeNull();
	});

	it('shows the follow button for somebody else', () => {
		vi.stubGlobal('fetch', routes());
		render(ProfilePage, {
			params: {} as never,
			data: data({
				overview: {
					...fixtures.overview,
					own: false,
					slug: 'work',
					name: 'Work',
					visibility: 'followers'
				},
				follow: {
					username: 'fiona',
					slug: 'work',
					name: 'Work',
					visibility: 'followers',
					own: false,
					state: 'none'
				}
			})
		});
		expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(
			'fiona / Work Followers only'
		);
		expect(screen.getByRole('button', { name: 'Ask to follow' })).toBeInTheDocument();
	});

	it('compares somebody else’s profile with the viewer’s, for the same period', () => {
		vi.stubGlobal('fetch', routes());
		render(ProfilePage, {
			params: {} as never,
			data: data({
				me: { ...fixtures.me, username: 'alex' },
				overview: { ...fixtures.overview, own: false },
				period: { kind: 'year', year: 2010 },
				source: 'Navidrome'
			})
		});
		expect(screen.getByRole('link', { name: 'Compare' })).toHaveAttribute(
			'href',
			'/u/fiona/compare/alex?year=2010&source=Navidrome'
		);
	});

	it('follows listens and now playing live', async () => {
		const playing: NowPlaying = {
			artist: 'Die Ärzte',
			track: 'Junge',
			album: 'Jazz ist anders',
			started_at: '2026-10-07T09:59:00Z',
			duration_ms: 180_000
		};
		let nowPlaying: NowPlaying | null = playing;
		const fetch = routes({
			[`GET ${api}/now-playing`]: () => nowPlaying,
			[`GET ${api}/listens`]: {
				listens: [
					listen('2026-10-07T09:58:00Z', { track: 'Junge', recording_id: 30 }),
					listen('2026-10-07T09:55:00Z')
				],
				next: null
			}
		});
		vi.stubGlobal('fetch', fetch);
		render(ProfilePage, { params: {} as never, data: data() });
		const [events] = FakeEventSource.all;
		expect(events.url).toBe(`${api}/live`);
		events.onopen?.();
		events.emit('now-playing');
		expect(await screen.findByText('Now playing')).toBeInTheDocument();
		expect(screen.getByText(/Die Ärzte · Jazz ist anders/)).toBeInTheDocument();
		nowPlaying = null;
		events.emit('listens');
		expect(await screen.findByRole('link', { name: 'Junge' })).toBeInTheDocument();
		await vi.waitFor(() => expect(screen.queryByText('Now playing')).toBeNull());
		// Only new listens go on top.
		expect(screen.getAllByRole('link', { name: 'Schrei nach Liebe' })).toHaveLength(1);
	});

	it('lets go of the connection in a hidden tab and catches up after', async () => {
		const fetch = routes({
			[`GET ${api}/now-playing`]: null,
			[`GET ${api}/listens`]: { listens: [], next: null }
		});
		vi.stubGlobal('fetch', fetch);
		const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(false);
		const { unmount } = render(ProfilePage, { params: {} as never, data: data() });
		const [first] = FakeEventSource.all;
		hidden.mockReturnValue(true);
		document.dispatchEvent(new Event('visibilitychange'));
		expect(first.closed).toBe(true);
		hidden.mockReturnValue(false);
		document.dispatchEvent(new Event('visibilitychange'));
		const second = FakeEventSource.all[1];
		second.onopen?.();
		await vi.waitFor(() => expect(fetch).toHaveBeenCalledWith(`${api}/listens?limit=25`));
		unmount();
		expect(second.closed).toBe(true);
	});

	it('ends now playing after the track', async () => {
		vi.useFakeTimers({ now: new Date('2026-10-07T10:00:00Z') });
		const fetch = routes({ [`GET ${api}/now-playing`]: null });
		vi.stubGlobal('fetch', fetch);
		render(ProfilePage, {
			params: {} as never,
			data: data({
				nowPlaying: {
					artist: 'Björk',
					track: 'Jóga',
					album: null,
					started_at: '2026-10-07T09:59:00Z',
					duration_ms: null
				}
			})
		});
		expect(screen.getByText('Now playing')).toBeInTheDocument();
		await vi.advanceTimersByTimeAsync(10 * 60_000);
		expect(screen.queryByText('Now playing')).toBeNull();
	});

	it('keeps going when the server is away', async () => {
		const fetch = fakeApi({});
		vi.stubGlobal('fetch', fetch);
		render(ProfilePage, { params: {} as never, data: data() });
		const [events] = FakeEventSource.all;
		events.emit('now-playing');
		events.emit('listens');
		await vi.waitFor(() => expect(fetch).toHaveBeenCalledTimes(3));
		expect(screen.getByRole('heading', { name: 'Recent listens' })).toBeInTheDocument();
	});
});

describe('follow page', () => {
	const info = {
		username: 'bela',
		slug: 'work',
		name: 'Work',
		visibility: 'followers' as const,
		own: false,
		state: 'none' as const
	};

	it('lets the viewer ask and goes in once followed', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({ 'PUT /api/profiles/bela/work/follow': { ...info, state: 'following' } })
		);
		render(FollowPage, { params: {} as never, data: { me: fixtures.me, info } } as never);
		expect(screen.getByText(/bela shows this profile to followers only/)).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Ask to follow' }));
		expect(goto).toHaveBeenCalledWith('/u/bela/work');
	});

	it('waits for a yes', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({ 'PUT /api/profiles/bela/work/follow': { ...info, state: 'requested' } })
		);
		render(FollowPage, { params: {} as never, data: { me: fixtures.me, info } } as never);
		await userEvent.click(screen.getByRole('button', { name: 'Ask to follow' }));
		expect(await screen.findByText('Asked bela to follow')).toBeInTheDocument();
		expect(goto).not.toHaveBeenCalled();
	});

	it('links the profile for who may see it', () => {
		render(FollowPage, {
			params: {} as never,
			data: { me: fixtures.me, info: { ...info, slug: 'default', state: 'following' } }
		} as never);
		expect(screen.getByRole('link', { name: 'Go to the profile' })).toHaveAttribute(
			'href',
			'/u/bela'
		);
	});
});

describe('search page', () => {
	const data = (q: string, found = { artists: fixtures.artists, releases: [], recordings: [] }) =>
		({
			me: null,
			base: '/u/fiona/work',
			overview: { ...fixtures.overview, slug: 'work', name: 'Work' },
			q,
			source: 'Navidrome',
			found,
			sources: fixtures.sources
		}) as never;

	it('explains itself without a query', () => {
		render(SearchPage, { params: {} as never, data: data('') });
		expect(screen.getByText(/Every word counts/)).toBeInTheDocument();
		expect(screen.getByRole('link', { name: /^fiona\s\/ Work$/ })).toHaveAttribute(
			'href',
			'/u/fiona/work?source=Navidrome'
		);
	});

	it('shows what it found, linked with the source', () => {
		render(SearchPage, { params: {} as never, data: data('ärzte') });
		expect(screen.getByRole('link', { name: /Die Ärzte/ })).toHaveAttribute(
			'href',
			'/u/fiona/work/artist/1-die-arzte?source=Navidrome'
		);
		expect(screen.getByRole('heading', { name: 'Albums' }).parentElement).toHaveTextContent(
			'Nothing here.'
		);
	});

	it('says when nothing matches', () => {
		render(SearchPage, {
			params: {} as never,
			data: data('xyz', { artists: [], releases: [], recordings: [] })
		});
		expect(screen.getByText('Nothing heard matches “xyz”.')).toBeInTheDocument();
	});

	it('puts the query into the address while typing', async () => {
		vi.useFakeTimers({ now: new Date('2026-10-07T10:00:00Z') });
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		render(SearchPage, { params: {} as never, data: data('') });
		const field = screen.getByRole('searchbox');
		await user.type(field, 'die ärzte');
		await vi.advanceTimersByTimeAsync(300);
		expect(goto).toHaveBeenCalledTimes(1);
		expect(goto).toHaveBeenCalledWith('/u/fiona/work/search?q=die+%C3%A4rzte&source=Navidrome', {
			replace: true,
			reset: false
		});
		await user.clear(field);
		await user.keyboard('{Enter}');
		await vi.advanceTimersByTimeAsync(300);
		expect(goto).toHaveBeenLastCalledWith('/u/fiona/work/search?source=Navidrome', {
			replace: true,
			reset: false
		});
	});
});

describe('artist, album and track pages', () => {
	const data = (kind: string, fields: Record<string, unknown> = {}) =>
		({
			me: null,
			base: '/u/fiona',
			kind,
			source: null,
			sources: [],
			entity: { ...fixtures.entity, ...fields }
		}) as never;

	it('shows an artist with its albums and tracks', () => {
		layOut(600);
		render(EntityPage, { params: {} as never, data: data('artist') });
		expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Die Ärzte');
		expect(screen.getByRole('link', { name: 'MusicBrainz' })).toHaveAttribute(
			'href',
			'https://musicbrainz.org/artist/f2fb0ff0-5679-42ec-a55c-15109ce6e320'
		);
		expect(screen.getByText('800')).toBeInTheDocument();
		expect(screen.getByText('Aug 12, 2007')).toBeInTheDocument();
		expect(screen.getByText(/Aug 2007 – Sep 2007/)).toHaveTextContent('· 800 listens (100%)');
		const albums = screen.getByRole('heading', { name: 'Albums' }).parentElement!;
		expect(within(albums).getByRole('link')).toHaveAttribute(
			'href',
			'/u/fiona/album/10-die-bestie-in-menschengestalt'
		);
		// The artist's own name is left out of its lists.
		expect(albums).not.toHaveTextContent('· Die Ärzte');
		expect(screen.getByRole('heading', { name: 'Tracks' })).toBeInTheDocument();
	});

	it('shows an album by its artist', () => {
		render(EntityPage, {
			params: {} as never,
			data: data('album', {
				name: 'Die Bestie in Menschengestalt',
				artist: { id: 1, name: 'Die Ärzte' },
				mbids: ['a', 'b'],
				phases: [{ from: '2007-08', to: '2007-08', listens: 300 }]
			})
		});
		expect(screen.getByRole('link', { name: 'Die Ärzte' })).toHaveAttribute(
			'href',
			'/u/fiona/artist/1-die-arzte'
		);
		expect(screen.getByRole('link', { name: 'MusicBrainz 2 of 2' })).toHaveAttribute(
			'href',
			'https://musicbrainz.org/release-group/b'
		);
		expect(screen.getByText('Aug 2007')).toBeInTheDocument();
		expect(screen.queryByRole('heading', { name: 'Albums' })).toBeNull();
	});

	it('shows album covers on artist and track pages, none for an artist itself', () => {
		useCovers(true);
		const artist = render(EntityPage, {
			params: {} as never,
			data: { ...(data('artist') as object), api } as never
		});
		const albums = screen.getByRole('heading', { name: 'Albums' }).parentElement!;
		expect(albums.querySelector('img')).toHaveAttribute('src', `${api}/releases/10/cover`);
		// No large cover for an artist.
		expect(artist.container.querySelectorAll('img').length).toBe(1);
		artist.unmount();
		render(EntityPage, {
			params: {} as never,
			data: {
				...(data('track', { name: 'Junge', artist: { id: 1, name: 'Die Ärzte' } }) as object),
				api
			} as never
		});
		const on = screen.getByRole('heading', { name: 'On albums' }).parentElement!;
		expect(on.querySelector('img')).toHaveAttribute('src', `${api}/releases/10/cover`);
	});

	it('shows a track that was never heard here', () => {
		render(EntityPage, {
			params: {} as never,
			data: data('track', {
				name: 'Junge',
				mbids: [],
				listens: 0,
				first_listened_at: null,
				last_listened_at: null,
				phases: [],
				entity: { profile: { username: 'fiona', slug: 'work', name: 'Work' } }
			})
		});
		expect(screen.getByText('Not heard in this profile.')).toBeInTheDocument();
		expect(screen.queryByText('MusicBrainz')).toBeNull();
		expect(screen.getByRole('heading', { name: 'On albums' })).toBeInTheDocument();
		expect(screen.queryByText('First listen')).toBeNull();
	});

	it('names the profile when it is not the default one', () => {
		render(EntityPage, {
			params: {} as never,
			data: data('track', {
				profile: { username: 'fiona', slug: 'work', name: 'Work' },
				phases: []
			})
		});
		expect(screen.getByRole('link', { name: /^fiona\s\/ Work$/ })).toBeInTheDocument();
		expect(screen.queryByText('Heavy listening')).toBeNull();
	});
});

describe('year in review', () => {
	const data = (fields: Record<string, unknown> = {}, overview = fixtures.overview) =>
		({
			me: null,
			base: '/u/fiona',
			source: 'Navidrome',
			sources: [],
			overview,
			review: { ...fixtures.review, ...fields },
			artists: fixtures.artists,
			releases: fixtures.releases,
			recordings: fixtures.recordings
		}) as never;

	it('sums up a year', () => {
		render(YearPage, { params: {} as never, data: data() });
		expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('2010 in review');
		expect(screen.getByRole('link', { name: '← 2007' })).toHaveAttribute(
			'href',
			'/u/fiona/year/2007?source=Navidrome'
		);
		expect(screen.getByRole('link', { name: '2026 →' })).toBeInTheDocument();
		expect(screen.getByText('50% more than in 2009')).toBeInTheDocument();
		expect(screen.getByText(/30 new, 400\s+tracks/)).toBeInTheDocument();
		expect(screen.getByText('Days with listens').nextElementSibling).toHaveTextContent(
			'200 of 365'
		);
		expect(screen.getByText('July')).toBeInTheDocument();
		expect(screen.getByText('Jul 1')).toBeInTheDocument();
		expect(screen.getByText('3 h 56 min')).toBeInTheDocument();
		expect(screen.getByText(/· 3 h 56 min · 61 listens/)).toBeInTheDocument();
		expect(
			screen.getByText(/30 artists heard for the first time, the most heard of them/)
		).toBeInTheDocument();
		expect(screen.getByText('· since Mar 27')).toBeInTheDocument();
		expect(screen.getByText('back')).toBeInTheDocument();
		expect(screen.getByText('#4 → #1')).toBeInTheDocument();
		expect(screen.getByText('+200')).toBeInTheDocument();
		expect(screen.getByText(/A session goes on/)).toHaveTextContent(
			'Mostly Die Ärzte (40), Björk (15), and Tocotronic (6).'
		);
	});

	it('shows album covers in the top albums when the server has them', () => {
		useCovers(true);
		const { container } = render(YearPage, { params: {} as never, data: data() });
		const albums = screen.getByRole('heading', { name: 'Top albums' }).parentElement!;
		expect(albums.querySelector('img')).toHaveAttribute(
			'src',
			expect.stringMatching(/\/releases\/10\/cover$/)
		);
		expect(container.querySelectorAll('img').length).toBe(albums.querySelectorAll('img').length);
	});

	it('tells a running year from a past one', () => {
		render(YearPage, {
			params: {} as never,
			data: data(
				{
					year: 2026,
					complete: false,
					last_year_so_far: 0,
					recordings: 1,
					new_artists: 1,
					discoveries: [],
					risers: [],
					top_day: null,
					longest_session: {
						started_at: '2026-01-01T12:00:00Z',
						ended_at: '2026-01-01T12:00:30Z',
						listens: 1,
						artists: []
					}
				},
				{ ...fixtures.overview, slug: 'work', name: 'Work' }
			)
		});
		expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('2026 in review so far');
		expect(screen.getByText('none in 2025 by now')).toBeInTheDocument();
		expect(screen.getByText(/1 new, 1\s+track$/)).toBeInTheDocument();
		expect(screen.getByText('Days with listens').nextElementSibling).toHaveTextContent('of 280');
		expect(screen.getByText('No artists heard for the first time.')).toBeInTheDocument();
		expect(
			screen.getByText('No artist heard before got more listens than in 2025 by now.')
		).toBeInTheDocument();
		expect(screen.getByText('Longest session').nextElementSibling).toHaveTextContent('1 listen');
		expect(screen.queryByRole('link', { name: /2026 →/ })).toBeNull();
		expect(screen.getByRole('link', { name: /^fiona\s\/ Work$/ })).toHaveAttribute(
			'href',
			'/u/fiona?year=2026&source=Navidrome'
		);
	});

	it('compares like for like', () => {
		const { unmount } = render(YearPage, {
			params: {} as never,
			data: data({ last_year_so_far: 901 })
		});
		expect(screen.getByText('as many as in 2009')).toBeInTheDocument();
		unmount();
		render(YearPage, {
			params: {} as never,
			data: data({
				last_year_so_far: 1800,
				new_artists: 2,
				discoveries: [fixtures.review.discoveries[0], fixtures.review.discoveries[1]]
			})
		});
		expect(screen.getByText('50% fewer than in 2009')).toBeInTheDocument();
		expect(screen.getByText(/^2\s+artists heard for the first time$/)).toBeInTheDocument();
	});

	it('says when a year had no listens', () => {
		render(YearPage, { params: {} as never, data: data({ listens: 0 }) });
		expect(screen.getByText('No listens in 2010.')).toBeInTheDocument();
	});
});
