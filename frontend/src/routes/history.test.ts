// The page "Edit listening history" with its trash.

import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { HistoryListen } from '#lib/api.ts';
import { invalidateAll } from '../test/app.svelte.ts';
import { fakeApi } from '../test/fetch.ts';
import { me, sources } from '../test/fixtures.ts';
import HistoryPage from './settings/history/[slug]/+page.svelte';

vi.mock('$app/state', () => import('../test/app.svelte.ts'));
vi.mock('$app/navigation', () => import('../test/app.svelte.ts'));

const api = '/api/me/profiles/default';

const listen = (
	id: number,
	listened_at: string,
	fields: Partial<HistoryListen> = {}
): HistoryListen => ({
	id,
	listened_at,
	artist: 'Die Ärzte',
	track: `Track ${id}`,
	album: 'Geräusch',
	source: 'Navidrome',
	trashed_at: null,
	...fields
});

const listens = [
	listen(1, '2026-10-07T09:00:00Z'),
	listen(2, '2026-10-06T09:00:00Z', { album: null, source: '' })
];
const trashed = [listen(3, '2026-10-01T09:00:00Z', { trashed_at: '2026-10-07T08:00:00Z' })];

const empty = { q: '', from: '', to: '', source: '' };
const data = (fields: Record<string, unknown> = {}) =>
	({
		me,
		profile: me.profiles[0],
		api,
		trash: false,
		filter: empty,
		history: { listens, next: null, total: 2 },
		inTrash: { listens: trashed, next: null, total: 1 },
		sources,
		...fields
	}) as never;

beforeEach(() => {
	vi.useFakeTimers({ now: new Date('2026-10-07T10:00:00Z'), toFake: ['Date'] });
	invalidateAll.mockClear();
});
afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('listening history', () => {
	it('lists the listens by day with a filter form', () => {
		render(HistoryPage, { params: {} as never, data: data() });
		expect(screen.getByRole('link', { name: '/u/fiona' })).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'Listens' })).toHaveAttribute('aria-current', 'page');
		expect(screen.getByRole('link', { name: 'Trash 1' })).toBeInTheDocument();
		expect(screen.getByRole('option', { name: 'Old musicbanana' })).toBeInTheDocument();
		expect(screen.getByText('2 listens.')).toBeInTheDocument();
		expect(screen.getByText('Today')).toBeInTheDocument();
		expect(screen.getByText('Yesterday')).toBeInTheDocument();
		expect(screen.getByText('Unknown player')).toBeInTheDocument();
		expect(screen.getByText('· Die Ärzte · Geräusch')).toBeInTheDocument();
		expect(screen.queryByRole('link', { name: 'Clear' })).toBeNull();
	});

	it('says when nothing is there', () => {
		render(HistoryPage, {
			params: {} as never,
			data: data({ history: { listens: [], next: null, total: 0 } })
		});
		expect(screen.getByText('No listens.')).toBeInTheDocument();
	});

	it('moves selected listens to the trash and takes it back', async () => {
		const calls: [string, unknown][] = [];
		vi.stubGlobal(
			'fetch',
			fakeApi({
				[`POST ${api}/trash`]: (init: RequestInit) => {
					calls.push(['trash', JSON.parse(String(init.body))]);
					return { ids: [1] };
				},
				[`POST ${api}/trash/restore`]: (init: RequestInit) => {
					calls.push(['restore', JSON.parse(String(init.body))]);
					return { restored: 1, kept: 0 };
				}
			})
		);
		render(HistoryPage, { params: {} as never, data: data() });
		const move = screen.getByRole('button', { name: 'Move selected to the trash' });
		expect(move).toBeDisabled();
		await userEvent.click(screen.getByRole('checkbox', { name: /Track 1/ }));
		expect(screen.getByText(/1 selected\./)).toBeInTheDocument();
		await userEvent.click(move);
		expect(calls[0]).toEqual(['trash', { ids: [1] }]);
		expect(screen.getByRole('status')).toHaveTextContent('Moved 1 listen to the trash.');
		expect(invalidateAll).toHaveBeenCalled();
		await userEvent.click(screen.getByRole('button', { name: 'Undo' }));
		expect(calls[1]).toEqual(['restore', { ids: [1] }]);
		expect(screen.getByRole('status')).toHaveTextContent('Restored 1 listen.');
	});

	it('selects all shown and back', async () => {
		render(HistoryPage, { params: {} as never, data: data() });
		const all = screen.getByRole('checkbox', { name: 'Select all shown' });
		await userEvent.click(all);
		expect(screen.getByText(/2 selected\./)).toBeInTheDocument();
		expect(all).toBeChecked();
		await userEvent.click(screen.getByRole('checkbox', { name: /Track 2/ }));
		expect(all).not.toBeChecked();
		await userEvent.click(all);
		await userEvent.click(all);
		expect(screen.queryByText(/selected\./)).toBeNull();
	});

	it('moves everything that matches a filter, after asking', async () => {
		const trash = vi.fn((init: RequestInit) => {
			expect(JSON.parse(String(init.body))).toEqual({ q: 'ärzte', source: 'Navidrome' });
			return { ids: [] };
		});
		vi.stubGlobal('fetch', fakeApi({ [`POST ${api}/trash`]: trash }));
		const confirm = vi.fn(() => false);
		vi.stubGlobal('confirm', confirm);
		render(HistoryPage, {
			params: {} as never,
			data: data({ filter: { ...empty, q: 'ärzte', source: 'Navidrome' } })
		});
		expect(screen.getByText('2 listens match.')).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'Clear' })).toHaveAttribute('href', '?');
		const button = screen.getByRole('button', { name: 'Move all 2 to the trash' });
		await userEvent.click(button);
		expect(confirm).toHaveBeenCalledWith('Move all 2 listens that match to the trash?');
		expect(trash).not.toHaveBeenCalled();
		confirm.mockReturnValue(true);
		await userEvent.click(button);
		expect(trash).toHaveBeenCalled();
		expect(screen.getByRole('status')).toHaveTextContent('Moved 0 listens to the trash.');
		expect(screen.queryByRole('button', { name: 'Undo' })).toBeNull();
	});

	it('shows more with the filter', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({
				[`GET ${api}/history`]: (_: unknown, url: URL) => {
					expect(url.search).toBe('?q=x&before=cursor');
					return { listens: [listen(9, '2026-09-01T09:00:00Z')], next: null, total: 3 };
				}
			})
		);
		render(HistoryPage, {
			params: {} as never,
			data: data({
				filter: { ...empty, q: 'x' },
				history: { listens, next: 'cursor', total: 3 }
			})
		});
		await userEvent.click(screen.getByRole('button', { name: 'Show more' }));
		expect(screen.getByText('Track 9')).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Show more' })).toBeNull();
	});

	it('shows errors', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({ [`POST ${api}/trash`]: new Response('read only', { status: 503 }) })
		);
		render(HistoryPage, {
			params: {} as never,
			data: data({ history: { listens, next: 'cursor', total: 3 } })
		});
		await userEvent.click(screen.getByRole('button', { name: 'Show more' }));
		expect(screen.getByRole('alert')).toHaveTextContent(/No route/);
		await userEvent.click(screen.getByRole('checkbox', { name: /Track 1/ }));
		await userEvent.click(screen.getByRole('button', { name: 'Move selected to the trash' }));
		expect(screen.getByRole('alert')).toHaveTextContent('Read only');
	});
});

describe('trash', () => {
	const inTrash = (fields: Record<string, unknown> = {}) =>
		data({ trash: true, history: null, ...fields });

	it('lists what is in it, and when it went there', () => {
		render(HistoryPage, { params: {} as never, data: inTrash() });
		expect(screen.getByRole('link', { name: 'Trash 1' })).toHaveAttribute('aria-current', 'page');
		expect(screen.queryByRole('searchbox')).toBeNull();
		expect(screen.getByText('1 listen in the trash.')).toBeInTheDocument();
		expect(screen.getByText('2 hours ago')).toBeInTheDocument();
	});

	it('says when it is empty', () => {
		render(HistoryPage, {
			params: {} as never,
			data: inTrash({ inTrash: { listens: [], next: null, total: 0 } })
		});
		expect(screen.getByText('The trash is empty.')).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'Restore all' })).toBeDisabled();
		expect(screen.getByRole('button', { name: 'Empty the trash' })).toBeDisabled();
	});

	it('restores, telling what stayed', async () => {
		const restore = vi.fn((init: RequestInit) => {
			expect(JSON.parse(String(init.body))).toEqual({});
			return { restored: 1, kept: 2 };
		});
		vi.stubGlobal('fetch', fakeApi({ [`POST ${api}/trash/restore`]: restore }));
		render(HistoryPage, { params: {} as never, data: inTrash() });
		await userEvent.click(screen.getByRole('button', { name: 'Restore all' }));
		expect(screen.getByRole('status')).toHaveTextContent(
			'Restored 1 listen. 2 listens stayed in the trash: the profile has another listen at that time now.'
		);
	});

	it('restores and deletes selected ones', async () => {
		const bodies: unknown[] = [];
		const record = (answer: unknown) => (init: RequestInit) => {
			bodies.push(JSON.parse(String(init.body)));
			return answer;
		};
		vi.stubGlobal(
			'fetch',
			fakeApi({
				[`POST ${api}/trash/restore`]: record({ restored: 1, kept: 0 }),
				[`POST ${api}/trash/empty`]: record({ deleted: 1 })
			})
		);
		vi.stubGlobal('confirm', () => true);
		render(HistoryPage, { params: {} as never, data: inTrash() });
		await userEvent.click(screen.getByRole('checkbox', { name: /Track 3/ }));
		await userEvent.click(screen.getByRole('button', { name: 'Restore selected' }));
		expect(bodies[0]).toEqual({ ids: [3] });
		await userEvent.click(screen.getByRole('checkbox', { name: /Track 3/ }));
		await userEvent.click(screen.getByRole('button', { name: 'Delete selected for good' }));
		expect(bodies[1]).toEqual({ ids: [3] });
		expect(screen.getByRole('status')).toHaveTextContent('Deleted 1 listen for good.');
	});

	it('empties the trash only when sure', async () => {
		const empty = vi.fn(() => ({ deleted: 1 }));
		vi.stubGlobal('fetch', fakeApi({ [`POST ${api}/trash/empty`]: empty }));
		const confirm = vi.fn(() => false);
		vi.stubGlobal('confirm', confirm);
		render(HistoryPage, { params: {} as never, data: inTrash() });
		await userEvent.click(screen.getByRole('button', { name: 'Empty the trash' }));
		expect(confirm).toHaveBeenCalledWith("Delete 1 listen for good? This can't be undone.");
		expect(empty).not.toHaveBeenCalled();
	});

	it('shows more of the trash', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({
				[`GET ${api}/trash`]: (_: unknown, url: URL) => {
					expect(url.search).toBe('?before=cursor');
					return {
						listens: [listen(4, '2026-09-01T09:00:00Z', { trashed_at: '2026-10-01T00:00:00Z' })],
						next: null,
						total: 2
					};
				}
			})
		);
		render(HistoryPage, {
			params: {} as never,
			data: inTrash({ inTrash: { listens: trashed, next: 'cursor', total: 2 } })
		});
		await userEvent.click(screen.getByRole('button', { name: 'Show more' }));
		expect(screen.getByText('Track 4')).toBeInTheDocument();
	});
});
