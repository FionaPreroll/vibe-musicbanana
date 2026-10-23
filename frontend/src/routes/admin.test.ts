// The admin pages: status and merges.

import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { MergeLog, MergeResult, MergeSuggestions, Status } from '#lib/api.ts';
import { invalidateAll } from '../test/app.svelte.ts';
import { fakeApi } from '../test/fetch.ts';
import { admin } from '../test/fixtures.ts';
import MergesPage from './merges/+page.svelte';
import StatusPage from './status/+page.svelte';

vi.mock('$app/state', () => import('../test/app.svelte.ts'));
vi.mock('$app/navigation', () => import('../test/app.svelte.ts'));

beforeEach(() => invalidateAll.mockClear());
afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('status page', () => {
	const status = (fields: Partial<Status> = {}): Status => ({
		build: {
			version: '0.1.0',
			commit: 'b3bc6e1a2b3c4d5e',
			debug: false,
			started_at: '2026-10-07T08:00:00Z',
			uptime_seconds: 2 * 86_400 + 3 * 3_600
		},
		process: {
			pid: 1,
			threads: 12,
			memory_bytes: 50 * 1024 * 1024,
			memory_peak_bytes: 1536 * 1024 * 1024,
			cpu_seconds: 1234.4,
			cpu_percent: 2.5,
			cores: 4,
			load_average: [0.5, 0.25, 0.125],
			memory_total_bytes: 8 * 1024 ** 3,
			memory_available_bytes: 4 * 1024 ** 3,
			memory_limit_bytes: 2 * 1024 ** 3
		},
		database: {
			version: 'PostgreSQL 16.4 on x86_64-pc-linux-gnu',
			bytes: 101 * 1024 * 1024,
			migration: 17,
			pool_size: 10,
			pool_idle: 7,
			connections: 12,
			cache_hit_percent: 99.9,
			tables: [{ name: 'listen', rows: 225_000, bytes: 900 }],
			last_day: [{ source: 'Spotify via YourSpotify', listens: 1 }]
		},
		workers: {
			last_round_at: '2026-10-07T09:59:00Z',
			yourspotify: [
				{
					id: 3,
					username: 'fiona',
					profile: 'default',
					url: 'https://yourspotify.example',
					created_at: '2026-10-01T00:00:00Z',
					started_at: '2026-10-07T09:00:00Z',
					finished_at: '2026-10-07T09:01:00Z',
					imported: 5000,
					error: 'timeout',
					refetch_requested_at: '2026-10-07T08:00:00Z',
					refetch_at: null,
					refetch_plays: 0,
					refetch_new: 0,
					refetch_left_out: 0,
					refetched_at: null
				}
			]
		},
		routes: [
			{
				route: 'GET /api/profiles/{username}/{slug}',
				requests: 1500,
				server_errors: 2,
				mean_ms: 12.34,
				p50_ms: 0.05,
				p95_ms: 4.56,
				max_ms: 250
			}
		],
		...fields
	});

	const fact = (name: string) =>
		screen.getByText(name, { selector: 'dt' }).nextElementSibling?.textContent;

	it('shows the numbers of the server', () => {
		render(StatusPage, { params: {} as never, data: { me: admin, status: status() } } as never);
		expect(fact('Commit')).toBe('b3bc6e1a2b3c');
		expect(fact('Uptime')).toBe('2 d 3 h');
		expect(fact('CPU')).toBe('2.5 % of a core (1,234 s in all)');
		expect(fact('Memory')).toBe('50.0 MB, at most 1.5 GB');
		expect(fact('Machine')).toBe('4 cores, load 0.50 0.25 0.13');
		expect(fact('Machine memory')).toBe('4.0 GB free of 8.0 GB, limit 2.0 GB');
		expect(fact('PostgreSQL')).toBe('PostgreSQL 16.4');
		expect(fact('Size')).toBe('101 MB');
		expect(fact('Connections')).toBe('3 busy, 7 idle in the pool; 12 to the database');
		expect(fact('Spotify')).toBe('1 listen');
		expect(
			screen.getByText(/last looked for due connections at Oct 7, 2026, 11:59 AM/)
		).toBeInTheDocument();
		expect(screen.getByText('Failed at Oct 7, 2026, 11:01 AM: timeout')).toHaveClass(
			'text-red-700'
		);
		expect(screen.getByText('Fetches everything again within a minute.')).toBeInTheDocument();
		const row = screen.getByText('GET /profiles/{username}/{slug}').closest('tr')!;
		expect([...row.querySelectorAll('td')].map((td) => td.textContent)).toEqual([
			'GET /profiles/{username}/{slug}',
			'1,500',
			'2',
			'12 ms',
			'< 0.1 ms',
			'4.6 ms',
			'250 ms'
		]);
		expect(screen.getByText('listen').closest('tr')).toHaveTextContent('225,000900 B');
	});

	it('copes with what a server cannot tell', () => {
		const s = status();
		render(StatusPage, {
			params: {} as never,
			data: {
				me: admin,
				status: {
					...s,
					build: { ...s.build, commit: null, debug: true, uptime_seconds: 3_725 },
					process: {
						...s.process,
						cpu_percent: null,
						threads: null,
						memory_bytes: null,
						load_average: null,
						memory_limit_bytes: null
					},
					database: {
						...s.database,
						migration: null,
						cache_hit_percent: null,
						connections: null,
						last_day: []
					},
					workers: { last_round_at: null, yourspotify: [] }
				}
			}
		} as never);
		expect(fact('Version')).toBe('0.1.0 (debug build)');
		expect(fact('Commit')).toBe('not known (local build)');
		expect(fact('Uptime')).toBe('1 h 2 min');
		expect(fact('CPU')).toBe('–');
		expect(fact('Threads')).toBe('–');
		expect(fact('Memory')).toBe('–, at most 1.5 GB');
		expect(fact('Machine')).toBe('4 cores, load –');
		expect(fact('Machine memory')).toBe('4.0 GB free of 8.0 GB');
		expect(fact('Migration')).toBe('–');
		expect(fact('Cache hits')).toBe('–');
		expect(screen.getAllByText('None.')).toHaveLength(2);
		expect(screen.getByText(/last looked for due connections not yet/)).toBeInTheDocument();
	});

	it('shows short uptimes in minutes and seconds', () => {
		const s = status();
		render(StatusPage, {
			params: {} as never,
			data: { me: admin, status: { ...s, build: { ...s.build, uptime_seconds: 65 } } }
		} as never);
		expect(fact('Uptime')).toBe('1 min 5 s');
	});

	it('refreshes every 10 seconds', async () => {
		vi.useFakeTimers();
		render(StatusPage, { params: {} as never, data: { me: admin, status: status() } } as never);
		await vi.advanceTimersByTimeAsync(25_000);
		expect(invalidateAll).toHaveBeenCalledTimes(2);
	});
});

describe('merges page', () => {
	const entry = (id: number, name: string, listens = 10) => ({ id, name, artist: null, listens });
	const suggestions = (fields: Partial<MergeSuggestions> = {}): MergeSuggestions => ({
		total: 3,
		hidden: 1,
		suggestions: [
			{
				from: entry(2, 'Die Aerzte', 3),
				into: entry(1, 'Die Ärzte', 800),
				likeness: 'same_letters'
			},
			{ from: entry(4, 'Bjork'), into: entry(3, 'Björk'), likeness: 'one_letter' }
		],
		...fields
	});
	const log = (fields: Partial<MergeLog> = {}): MergeLog => ({
		merges: [
			{
				id: 7,
				kind: 'artist',
				from: [5, 'Tocotronik'],
				into: [6, 'Tocotronic'],
				merged_at: '2026-10-06T12:00:00Z',
				undone_at: null
			},
			{
				id: 6,
				kind: 'recording',
				from: [8, 'Junge (Live)'],
				into: [9, 'Junge'],
				merged_at: '2026-10-05T12:00:00Z',
				undone_at: '2026-10-05T13:00:00Z'
			}
		],
		next: 6,
		...fields
	});
	const data = (fields: Record<string, unknown> = {}) =>
		({ me: admin, kind: 'artist', suggestions: suggestions(), log: log(), ...fields }) as never;

	const result = (fields: Partial<MergeResult> = {}): MergeResult => ({
		kind: 'artist',
		from: [2, 'Die Aerzte'],
		into: [1, 'Die Ärzte'],
		dry_run: true,
		listens: 3,
		spellings: 1,
		releases_moved: 1,
		releases_merged: 2,
		recordings_moved: 3,
		recordings_merged: 4,
		told_apart: false,
		op: null,
		...fields
	});

	it('lists suggestions and the log', () => {
		render(MergesPage, { params: {} as never, data: data() });
		expect(screen.getByRole('link', { name: 'Artists' })).toHaveAttribute('aria-current', 'page');
		expect(screen.getByText(/The first 2 of 3, the most certain first/)).toBeInTheDocument();
		expect(screen.getByText('Same letters')).toBeInTheDocument();
		expect(screen.getByText('· 800 listens')).toBeInTheDocument();
		expect(screen.getByText('#7').nextElementSibling).toHaveTextContent(
			/^artist\s*“Tocotronik”\s*#5\s*→\s*“Tocotronic”\s*#6$/
		);
		expect(screen.getByText('undone')).toHaveAttribute('title', 'Oct 5, 2026, 3:00 PM');
	});

	it('words the suggestions by kind and count', () => {
		const { unmount } = render(MergesPage, {
			params: {} as never,
			data: data({ kind: 'release', suggestions: suggestions({ total: 2, hidden: 0 }) })
		});
		expect(screen.getByText(/2 look-alikes, the most certain first/)).toBeInTheDocument();
		expect(screen.getByText(/only compared within an artist/)).toBeInTheDocument();
		expect(
			screen.getByText("Any two albums, also ones the suggestions don't find.")
		).toBeInTheDocument();
		unmount();
		render(MergesPage, {
			params: {} as never,
			data: data({
				suggestions: { total: 0, hidden: 0, suggestions: [] },
				log: { merges: [], next: null }
			})
		});
		expect(screen.getByText(/No look-alikes left/)).toBeInTheDocument();
		expect(screen.getByText('No merges yet.')).toBeInTheDocument();
	});

	it('previews and merges a suggestion, then offers to undo it', async () => {
		const posted: unknown[] = [];
		vi.stubGlobal(
			'fetch',
			fakeApi({
				'POST /api/admin/merges': (init: RequestInit) => {
					const body = JSON.parse(String(init.body));
					posted.push(body);
					return body.dry_run ? result() : result({ dry_run: false, op: 8 });
				},
				'POST /api/admin/merges/8/undo': (init: RequestInit) => {
					const body = JSON.parse(String(init.body));
					const merge = {
						...log().merges[0],
						id: 8,
						from: [2, 'Die Aerzte'],
						into: [1, 'Die Ärzte']
					};
					return { merge, restored: 4, kept: body.dry_run ? 1 : 0, dry_run: !!body.dry_run };
				}
			})
		);
		render(MergesPage, { params: {} as never, data: data() });
		await userEvent.click(screen.getAllByRole('button', { name: 'Merge…' })[0]);
		expect(screen.getByText(/Would merge artist/)).toHaveTextContent(
			'Would merge artist “Die Aerzte” (#2) into “Die Ärzte” (#1).'
		);
		expect(screen.getByText(/Albums: 1 moved over, 2 merged/)).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Merge' }));
		expect(posted[1]).toEqual({ kind: 'artist', from: 2, into: 1, force: false });
		expect(screen.getByRole('status')).toHaveTextContent(
			'Merged artist “Die Aerzte” into “Die Ärzte”: 3 listens moved over.'
		);
		expect(invalidateAll).toHaveBeenCalled();
		// The new merge is not in this log yet; the message offers the undo.
		await userEvent.click(within(screen.getByRole('status')).getByRole('button', { name: 'Undo' }));
		expect(posted).toHaveLength(2);
	});

	it('merges the other way, or cancels', async () => {
		const posted: { from: number; into: number }[] = [];
		vi.stubGlobal(
			'fetch',
			fakeApi({
				'POST /api/admin/merges': (init: RequestInit) => {
					const body = JSON.parse(String(init.body));
					posted.push(body);
					return result({ from: [body.from, 'x'], into: [body.into, 'y'] });
				}
			})
		);
		render(MergesPage, { params: {} as never, data: data() });
		await userEvent.click(screen.getAllByRole('button', { name: 'Other way…' })[1]);
		expect(posted[0]).toMatchObject({ from: 3, into: 4 });
		await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(screen.queryByText(/Would merge/)).toBeNull();
	});

	it('asks before merging what MusicBrainz tells apart', async () => {
		const posted: unknown[] = [];
		vi.stubGlobal(
			'fetch',
			fakeApi({
				'POST /api/admin/merges': (init: RequestInit) => {
					posted.push(JSON.parse(String(init.body)));
					return result({ told_apart: true, kind: 'recording', op: null });
				}
			})
		);
		render(MergesPage, { params: {} as never, data: data({ kind: 'recording' }) });
		await userEvent.click(screen.getAllByRole('button', { name: 'Merge…' })[0]);
		expect(screen.queryByText(/Albums:/)).toBeNull();
		const merge = screen.getByRole('button', { name: 'Merge' });
		expect(merge).toBeDisabled();
		await userEvent.click(screen.getByRole('checkbox'));
		await userEvent.click(merge);
		expect(posted[1]).toMatchObject({ force: true });
		expect(screen.getByRole('status')).not.toHaveTextContent('Undo');
	});

	it('shows errors of a preview and of a merge', async () => {
		let failMerge = false;
		vi.stubGlobal(
			'fetch',
			fakeApi({
				'POST /api/admin/merges': (init: RequestInit) =>
					JSON.parse(String(init.body)).dry_run && !failMerge
						? result()
						: new Response('merged meanwhile', { status: 409 })
			})
		);
		render(MergesPage, { params: {} as never, data: data() });
		await userEvent.click(screen.getAllByRole('button', { name: 'Merge…' })[0]);
		await userEvent.click(screen.getByRole('button', { name: 'Merge' }));
		expect(screen.getByRole('alert')).toHaveTextContent('Merged meanwhile');
		failMerge = true;
		await userEvent.click(screen.getAllByRole('button', { name: 'Merge…' })[0]);
		expect(screen.queryByText(/Would merge/)).toBeNull();
		expect(screen.getByRole('alert')).toHaveTextContent('Merged meanwhile');
	});

	it('hides a suggestion and shows the hidden ones', async () => {
		let hidden = [{ from: [10, 'Live'], into: [11, 'Leben'], hidden_at: '2026-10-01T10:00:00Z' }];
		vi.stubGlobal(
			'fetch',
			fakeApi({
				'POST /api/admin/merges/hidden/artist': (init: RequestInit) => {
					expect(JSON.parse(String(init.body))).toEqual({ from: 2, into: 1 });
					return undefined;
				},
				'GET /api/admin/merges/hidden/artist': () => hidden,
				'DELETE /api/admin/merges/hidden/artist/10/11': () => {
					hidden = [];
					return undefined;
				}
			})
		);
		render(MergesPage, { params: {} as never, data: data() });
		await userEvent.click(screen.getAllByRole('button', { name: 'Hide' })[0]);
		expect(screen.getByRole('status')).toHaveTextContent('Hidden.');
		await userEvent.click(screen.getByRole('button', { name: '1 hidden' }));
		expect(screen.getByText(/“Live” \(#10\) and “Leben” \(#11\)/)).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Suggest again' }));
		expect(screen.getByRole('status')).toHaveTextContent('“Live” and “Leben” are suggested again.');
		expect(screen.getByText('None.')).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Close the hidden ones' }));
		expect(screen.queryByText('None.')).toBeNull();
	});

	it('shows an error when the hidden ones do not load', async () => {
		vi.stubGlobal('fetch', fakeApi({}));
		render(MergesPage, { params: {} as never, data: data() });
		await userEvent.click(screen.getByRole('button', { name: '1 hidden' }));
		expect(screen.getByRole('alert')).toHaveTextContent(/No route/);
	});

	it('merges by hand', async () => {
		const posted: unknown[] = [];
		vi.stubGlobal(
			'fetch',
			fakeApi({
				'GET /api/admin/catalog/artist': (_: unknown, url: URL) =>
					url.searchParams.get('q') === 'toco'
						? [entry(5, 'Tocotronik', 2)]
						: [entry(6, 'Tocotronic', 200)],
				'POST /api/admin/merges': (init: RequestInit) => {
					const body = JSON.parse(String(init.body));
					posted.push(body);
					return result({
						from: [body.from, 'a'],
						into: [body.into, 'b'],
						op: body.dry_run ? null : 9
					});
				}
			})
		);
		render(MergesPage, { params: {} as never, data: data() });
		const preview = screen.getByRole('button', { name: 'Preview' });
		expect(preview).toBeDisabled();
		await userEvent.type(screen.getByRole('searchbox', { name: 'Merge (goes away)' }), 'toco');
		await userEvent.click(await screen.findByRole('button', { name: /Tocotronik/ }));
		await userEvent.type(screen.getByRole('searchbox', { name: 'Into (stays)' }), 'tocotronic');
		await userEvent.click(await screen.findByRole('button', { name: /Tocotronic ·/ }));
		await userEvent.click(screen.getByRole('button', { name: 'Swap' }));
		await userEvent.click(screen.getByRole('button', { name: 'Swap' }));
		await userEvent.click(preview);
		expect(posted[0]).toEqual({ kind: 'artist', from: 5, into: 6, dry_run: true });
		await userEvent.click(screen.getByRole('button', { name: 'Merge' }));
		expect(screen.getByRole('searchbox', { name: 'Merge (goes away)' })).toHaveValue('');
	});

	it('undoes a merge from the log', async () => {
		const undo = vi.fn((init: RequestInit) => {
			const dry = !!JSON.parse(String(init.body)).dry_run;
			return { merge: log().merges[0], restored: 1, kept: dry ? 2 : 0, dry_run: dry };
		});
		vi.stubGlobal('fetch', fakeApi({ 'POST /api/admin/merges/7/undo': undo }));
		render(MergesPage, { params: {} as never, data: data() });
		await userEvent.click(screen.getByRole('button', { name: 'Undo…' }));
		expect(screen.getByText(/Would undo merge 7/)).toHaveTextContent(
			'Would undo merge 7: artist “Tocotronik” stands on its own again, with 1 change put back. 2 rows stay as it is now, as it changed again since.'
		);
		await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(screen.queryByText(/Would undo/)).toBeNull();
		await userEvent.click(screen.getByRole('button', { name: 'Undo…' }));
		await userEvent.click(screen.getByRole('button', { name: 'Undo merge' }));
		expect(screen.getByRole('status')).toHaveTextContent(
			'Undid merge 7: artist “Tocotronik” stands on its own again.'
		);
		expect(undo).toHaveBeenCalledTimes(3);
	});

	it('shows when an undo cannot be previewed', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({
				'POST /api/admin/merges/7/undo': new Response('merged again since', { status: 409 })
			})
		);
		render(MergesPage, { params: {} as never, data: data() });
		await userEvent.click(screen.getByRole('button', { name: 'Undo…' }));
		expect(screen.getByRole('alert')).toHaveTextContent('Merged again since');
	});

	it('shows more of the log', async () => {
		let page = 0;
		vi.stubGlobal(
			'fetch',
			fakeApi({
				'GET /api/admin/merges': (_: unknown, url: URL) => {
					expect(url.searchParams.get('before')).toBe('6');
					return page++ === 0
						? {
								merges: [{ ...log().merges[0], id: 5, from: [1, 'Ärzte'] }],
								next: null
							}
						: new Response('', { status: 500 });
				}
			})
		);
		render(MergesPage, { params: {} as never, data: data() });
		await userEvent.click(screen.getByRole('button', { name: 'Show more' }));
		expect(screen.getByText('“Ärzte”')).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Show more' })).toBeNull();
	});

	it('shows an error when more of the log does not load', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({ 'GET /api/admin/merges': new Response('gone', { status: 500 }) })
		);
		render(MergesPage, { params: {} as never, data: data() });
		await userEvent.click(screen.getByRole('button', { name: 'Show more' }));
		expect(screen.getByRole('alert')).toHaveTextContent('Gone');
	});
});
