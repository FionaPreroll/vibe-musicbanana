import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { goto, visit } from '../../test/app.svelte.ts';
import PeriodPicker from './PeriodPicker.svelte';
import SourcePicker from './SourcePicker.svelte';

vi.mock('$app/state', () => import('../../test/app.svelte.ts'));
vi.mock('$app/navigation', () => import('../../test/app.svelte.ts'));

const source = (name: string, listens: number) => ({
	source: name,
	listens,
	first_listened_at: '2016-01-01T12:00:00Z',
	last_listened_at: '2026-10-01T12:00:00Z'
});

describe('SourcePicker', () => {
	it('stays away with one source', () => {
		visit('/u/fiona');
		render(SourcePicker, { sources: [source('Navidrome', 10)] });
		expect(screen.queryByRole('navigation')).toBeNull();
	});

	it('links each source and keeps the rest of the query', () => {
		visit('/u/fiona?year=2012');
		render(SourcePicker, {
			sources: [source('Navidrome', 10), source('Spotify via YourSpotify', 1)]
		});
		const nav = screen.getByRole('navigation', { name: 'Source' });
		const all = within(nav).getByRole('link', { name: 'All sources' });
		expect(all).toHaveAttribute('aria-current', 'page');
		expect(all).toHaveAttribute('href', '/u/fiona?year=2012');
		const spotify = within(nav).getByRole('link', { name: 'Spotify' });
		expect(spotify).toHaveAttribute('href', '/u/fiona?year=2012&source=Spotify+via+YourSpotify');
		expect(spotify).toHaveAttribute('title', '1 listen, Jan 1, 2016 to Oct 1, 2026');
	});

	it('offers the way back from a source', () => {
		visit('/u/fiona?source=Navidrome');
		render(SourcePicker, { sources: [source('Navidrome', 10)] });
		expect(screen.getByRole('link', { name: 'Navidrome' })).toHaveAttribute('aria-current', 'page');
		expect(screen.getByRole('link', { name: 'All sources' })).toHaveAttribute('href', '/u/fiona');
	});
});

describe('PeriodPicker', () => {
	const years = [
		{ year: 2012, listens: 400 },
		{ year: 2013, listens: 0 },
		{ year: 2014, listens: 100 }
	];
	const props = {
		years,
		first: '2012-03-01T10:00:00Z',
		last: '2014-11-30T10:00:00Z'
	};

	beforeEach(() => {
		Element.prototype.scrollIntoView = vi.fn();
		goto.mockClear();
	});
	afterEach(() => vi.useRealTimers());

	it('links the presets and years, keeping the source', () => {
		visit('/u/fiona?days=30&source=Navidrome');
		const { container } = render(PeriodPicker, { ...props, period: { kind: 'days', days: 30 } });
		expect(screen.getByRole('link', { name: 'All time' })).toHaveAttribute(
			'href',
			'/u/fiona?source=Navidrome'
		);
		const selected = screen.getByRole('link', { name: 'Last 30 days' });
		expect(selected).toHaveAttribute('aria-current', 'page');
		const year = screen.getByRole('link', { name: '2014' });
		expect(year).toHaveAttribute('href', '/u/fiona?year=2014&source=Navidrome');
		expect(year).toHaveAttribute('title', '100 listens');
		const bars = container.querySelectorAll<HTMLElement>('a span span:first-child');
		expect([...bars].map((b) => b.style.height)).toEqual(['100%', '0%', '25%']);
		expect(screen.queryByRole('form')).toBeNull();
	});

	it('shows the selected year', () => {
		visit('/u/fiona?year=2012');
		render(PeriodPicker, { ...props, period: { kind: 'year', year: 2012 } });
		expect(screen.getByRole('link', { name: '2012' })).toHaveAttribute('aria-current', 'page');
		expect(Element.prototype.scrollIntoView).toHaveBeenCalled();
	});

	it('picks a custom period', async () => {
		visit('/u/fiona');
		render(PeriodPicker, { ...props, period: { kind: 'all' } });
		const custom = screen.getByRole('button', { name: 'Custom' });
		await userEvent.click(custom);
		expect(custom).toHaveAttribute('aria-expanded', 'true');
		const from = screen.getByLabelText('From');
		const to = screen.getByLabelText('To');
		expect(from).toHaveValue('2012-03-01');
		expect(to).toHaveValue('2014-11-30');
		await userEvent.clear(to);
		await userEvent.click(screen.getByRole('button', { name: 'Show' }));
		expect(goto).toHaveBeenCalledWith('/u/fiona?from=2012-03-01', { reset: false });
	});

	it('goes back to all time without dates', async () => {
		visit('/u/fiona?from=2013-01-01');
		render(PeriodPicker, {
			years: [],
			first: null,
			last: null,
			period: { kind: 'range', from: '2013-01-01', to: null }
		});
		expect(screen.getByRole('button', { name: 'Since Jan 1, 2013' })).toHaveAttribute(
			'aria-expanded',
			'true'
		);
		await userEvent.clear(screen.getByLabelText('From'));
		await userEvent.click(screen.getByRole('button', { name: 'Show' }));
		expect(goto).toHaveBeenCalledWith('/u/fiona', { reset: false });
	});
});
