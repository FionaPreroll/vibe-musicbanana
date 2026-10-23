import { render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import Chart from './Chart.svelte';
import ListenTime from './ListenTime.svelte';
import VisibilityBadge from './VisibilityBadge.svelte';

describe('VisibilityBadge', () => {
	it('marks profiles not everybody sees', () => {
		const { rerender } = render(VisibilityBadge, { visibility: 'public' });
		expect(screen.queryByText(/Private|Followers/)).toBeNull();
		rerender({ visibility: 'private' });
		expect(screen.getByText('Private')).toHaveAttribute('title', 'Only you see this profile');
		rerender({ visibility: 'followers' });
		expect(screen.getByText('Followers only')).toBeInTheDocument();
	});
});

describe('Chart', () => {
	const entries = [
		{ id: 1, name: 'Schrei nach Liebe', artist: 'Die Ärzte', listens: 40 },
		{ id: 2, name: 'Westerland', artist: 'Die Ärzte', listens: 10 },
		{ id: 3, name: 'Jóga', artist: 'Björk', listens: 1200 }
	];

	it('says when it is empty', () => {
		render(Chart, { title: 'Top tracks', entries: [] });
		expect(screen.getByRole('heading', { name: 'Top tracks' })).toBeInTheDocument();
		expect(screen.getByText('Nothing here.')).toBeInTheDocument();
	});

	it('lists entries with bars relative to the first', () => {
		const { container } = render(Chart, { title: 'Top tracks', entries });
		const items = screen.getAllByRole('listitem');
		expect(items).toHaveLength(3);
		expect(items[0]).toHaveTextContent(/1\s*Schrei nach Liebe\s*· Die Ärzte\s*40/);
		expect(items[2]).toHaveTextContent('1,200');
		const bars = container.querySelectorAll<HTMLElement>('li > div:first-child');
		expect(bars[1].style.width).toBe('25%');
		expect(screen.queryByRole('link')).toBeNull();
	});

	it('links entries and hides the artist of the page', () => {
		render(Chart, {
			title: 'Top tracks',
			entries,
			href: (e) => `/track/${e.id}`,
			hideArtist: 'Die Ärzte'
		});
		const links = screen.getAllByRole('link');
		expect(links[1]).toHaveAttribute('href', '/track/2');
		expect(links[0]).not.toHaveTextContent('Die Ärzte');
		expect(links[2]).toHaveTextContent('Björk');
	});
});

describe('ListenTime', () => {
	afterEach(() => vi.useRealTimers());

	it('shows how long ago, or the time', () => {
		vi.useFakeTimers({ now: new Date('2026-10-07T10:00:00Z'), toFake: ['Date'] });
		const { rerender } = render(ListenTime, { at: '2026-10-07T09:55:00Z' });
		const time = screen.getByText('5 minutes ago');
		expect(time).toHaveAttribute('datetime', '2026-10-07T09:55:00Z');
		expect(time).toHaveAttribute('title', 'Oct 7, 2026, 11:55 AM');
		rerender({ at: '2026-10-07T01:00:00Z' });
		expect(screen.getByText('3:00 AM')).toBeInTheDocument();
		rerender({ at: '2026-10-07T01:00:00Z', ago: true });
		expect(screen.getByText('9 hours ago')).toBeInTheDocument();
	});

	it('keeps up with the time', async () => {
		vi.useFakeTimers({ now: new Date('2026-10-07T10:00:00Z') });
		render(ListenTime, { at: '2026-10-07T09:59:00Z' });
		expect(screen.getByText('1 minute ago')).toBeInTheDocument();
		await vi.advanceTimersByTimeAsync(60_000);
		expect(screen.getByText('2 minutes ago')).toBeInTheDocument();
	});
});
