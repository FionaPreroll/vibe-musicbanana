import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { ChartEntry } from '#lib/api.ts';
import Chart from './Chart.svelte';

// The places of Chart.svelte that come with `rank`, `previous_rank` and `more`;
// the plain chart is in small.test.ts.

const place = (id: number, rank = id, previous_rank?: number | null): ChartEntry => ({
	id,
	name: `Artist ${id}`,
	listens: 100 - id,
	rank,
	previous_rank
});
const top = (n: number, from = 1) => Array.from({ length: n }, (_, i) => place(from + i));

describe('Chart movement', () => {
	it('shows how each place moved', () => {
		const { container } = render(Chart, {
			title: 'Top artists',
			entries: [place(1, 1, 4), place(2, 2, 1), place(3, 2, 2), place(4, 4, null)]
		});
		const moves = [...container.querySelectorAll<HTMLElement>('[data-movement]')];
		expect(moves.map((m) => [m.dataset.movement, m.textContent?.trim()])).toEqual([
			['up', '↑3'],
			['down', '↓1'],
			['same', '='],
			['new', 'new']
		]);
		expect(moves.map((m) => m.title)).toEqual([
			'Up 3 places, from 4',
			'Down 1 place, from 1',
			'Same place as in the period before',
			'Not heard in the period before'
		]);
		// Places shared by entries with as many listens.
		const ranks = screen
			.getAllByRole('listitem')
			.map((li) => li.textContent?.trim().split(/\s/)[0]);
		expect(ranks).toEqual(['1', '2', '2', '4']);
	});

	it('shows no arrows without a comparison', () => {
		const { container } = render(Chart, { title: 'Top artists', entries: [place(1)] });
		expect(container.querySelector('[data-movement]')).toBeNull();
	});
});

describe('Chart "Show more"', () => {
	it('is only there with a way to load more and a full first page', () => {
		const more = vi.fn();
		const { rerender } = render(Chart, { title: 'Top artists', entries: top(10) });
		expect(screen.queryByRole('button')).toBeNull();
		rerender({ title: 'Top artists', entries: top(3), more });
		expect(screen.queryByRole('button')).toBeNull();
		rerender({ title: 'Top artists', entries: top(10), more });
		expect(screen.getByRole('button', { name: 'Show more' })).toBeInTheDocument();
	});

	it('adds places until there are no more, and shows less again', async () => {
		const user = userEvent.setup();
		const more = vi
			.fn()
			// One entry moved up between the pages and comes again.
			.mockResolvedValueOnce([place(10), ...top(2, 11)])
			.mockResolvedValueOnce([place(13)]);
		render(Chart, { title: 'Top artists', entries: top(10), more, pageSize: 3 });

		await user.click(screen.getByRole('button', { name: 'Show more' }));
		expect(more).toHaveBeenLastCalledWith(10, 3);
		await waitFor(() => expect(screen.getAllByRole('listitem')).toHaveLength(12));

		await user.click(screen.getByRole('button', { name: 'Show more' }));
		expect(more).toHaveBeenLastCalledWith(12, 3);
		await waitFor(() => expect(screen.getAllByRole('listitem')).toHaveLength(13));
		// A short page means the chart has ended.
		expect(screen.queryByRole('button', { name: 'Show more' })).toBeNull();

		await user.click(screen.getByRole('button', { name: 'Show less' }));
		expect(screen.getAllByRole('listitem')).toHaveLength(10);
		expect(screen.getByRole('button', { name: 'Show more' })).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Show less' })).toBeNull();
	});

	it('says while it loads and when loading failed', async () => {
		const user = userEvent.setup();
		let fail: (e: Error) => void = () => {};
		const more = vi.fn(() => new Promise<ChartEntry[]>((_, reject) => (fail = reject)));
		render(Chart, { title: 'Top artists', entries: top(10), more });

		await user.click(screen.getByRole('button', { name: 'Show more' }));
		expect(screen.getByRole('button', { name: 'Loading…' })).toBeDisabled();
		fail(new Error('offline'));
		expect(await screen.findByText('Could not load more.')).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'Show more' })).toBeEnabled();
		expect(screen.getAllByRole('listitem')).toHaveLength(10);
	});

	it('starts over with new entries and drops a page that arrives late', async () => {
		const user = userEvent.setup();
		let answer: (entries: ChartEntry[]) => void = () => {};
		const more = vi.fn(() => new Promise<ChartEntry[]>((resolve) => (answer = resolve)));
		const { rerender } = render(Chart, { title: 'Top artists', entries: top(10), more });

		await user.click(screen.getByRole('button', { name: 'Show more' }));
		// Another period is picked while the page loads.
		await rerender({ title: 'Top artists', entries: top(10, 50), more });
		answer(top(20, 11));
		await Promise.resolve();
		expect(screen.getAllByRole('listitem')).toHaveLength(10);
		expect(screen.getByRole('button', { name: 'Show more' })).toBeEnabled();
		expect(screen.getByText('Artist 50')).toBeInTheDocument();
	});
});
