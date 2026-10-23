import { fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Clock, OnThisDay as OnThisDayData, Week, YearTop } from '#lib/api.ts';
import { useTimeSettings } from '#lib/zone.svelte.ts';
import { visit } from '../../test/app.svelte.ts';
import { layOut } from '../../test/resize.ts';
import ArtistYears from './ArtistYears.svelte';
import ListeningClock from './ListeningClock.svelte';
import MonthCurve from './MonthCurve.svelte';
import OnThisDay from './OnThisDay.svelte';
import WeekGlance from './WeekGlance.svelte';

vi.mock('$app/state', () => import('../../test/app.svelte.ts'));

afterEach(() => {
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
	useTimeSettings(null);
});

const artist = (id: number, name: string, listens: number) => ({ id, name, listens });

describe('OnThisDay', () => {
	const year = (date: string, years_ago: number) => ({
		date,
		years_ago,
		listens: 12,
		artists: [artist(1, 'Die Ärzte', 8), artist(2, 'Björk', 4)],
		tracks: [
			{ id: 5, name: 'Westerland', artist: 'Die Ärzte', listens: 3 },
			{ id: 6, name: 'Jóga', artist: 'Björk', listens: 1 }
		]
	});
	const data: OnThisDayData = {
		day: '2026-10-07',
		years: [
			year('2025-10-07', 1),
			year('2024-10-07', 2),
			year('2016-10-07', 10),
			year('2010-10-07', 16)
		]
	};
	const props = {
		onThisDay: data,
		href: (kind: string, id: number) => `/${kind}/${id}`,
		dayHref: (day: string) => `/day/${day}`
	};

	it('shows three years, then all', async () => {
		render(OnThisDay, props);
		expect(screen.getByText('October 7 in earlier years')).toBeInTheDocument();
		expect(screen.getAllByRole('link', { name: /ago/ })).toHaveLength(3);
		const first = screen.getByRole('link', { name: /A year ago/ });
		expect(first).toHaveAttribute('href', '/day/2025-10-07');
		expect(first).toHaveTextContent('Tue, Oct 7, 2025');
		expect(first).toHaveTextContent('12 listens');
		await userEvent.click(screen.getByRole('button', { name: 'All 4 years' }));
		expect(screen.getByRole('link', { name: /16 years ago/ })).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Fewer years' }));
		expect(screen.queryByRole('link', { name: /16 years ago/ })).toBeNull();
	});

	it('links artists and tracks, with counts above one', () => {
		render(OnThisDay, { ...props, onThisDay: { ...data, years: data.years.slice(0, 1) } });
		expect(screen.getByRole('link', { name: 'Björk' })).toHaveAttribute('href', '/artist/2');
		expect(screen.getByRole('link', { name: 'Westerland' })).toHaveAttribute('href', '/track/5');
		expect(screen.getByText('3×')).toBeInTheDocument();
		expect(screen.queryByText('1×')).toBeNull();
		expect(screen.queryByRole('button')).toBeNull();
	});
});

describe('WeekGlance', () => {
	const days = ['05', '06', '07', '08', '09', '10', '11'].map((d, i) => ({
		date: `2026-10-${d}`,
		listens: i < 3 ? [10, 0, 5][i] : 0,
		last_week: 4
	}));
	const week = (fields: Partial<Week> = {}): Week => ({
		from: '2026-10-05',
		to: '2026-10-11',
		day: '2026-10-07',
		days,
		listens: 15,
		last_week_so_far: 12,
		last_week: 28,
		artists: [
			{ id: 1, name: 'Die Ärzte', listens: 10, last_week: 2, new: false },
			{ id: 2, name: 'Björk', listens: 5, last_week: 0, new: true }
		],
		new_artists: 1,
		streak: {
			current: 1,
			current_from: '2026-10-07',
			longest: 40,
			longest_from: '2012-01-01',
			longest_to: '2012-02-09'
		},
		...fields
	});
	const artistHref = (id: number) => `/artist/${id}`;

	it('sums up the week so far', () => {
		render(WeekGlance, { week: week(), artistHref });
		expect(screen.getByText('Mon, Oct 5 to Sun, Oct 11')).toBeInTheDocument();
		expect(screen.getByText('3 more than last week by now')).toBeInTheDocument();
		expect(screen.getByText('Days with listens').nextElementSibling).toHaveTextContent('2 of 3');
		expect(screen.getByText('1 day')).toBeInTheDocument();
		expect(screen.getByText('in a row since Oct 7, 2026')).toBeInTheDocument();
		expect(screen.getByText('40 days')).toBeInTheDocument();
		expect(screen.getByText('from Jan 1, 2012 to Feb 9, 2012')).toBeInTheDocument();
		const items = screen.getAllByRole('listitem');
		expect(items[3]).toHaveAttribute('title', 'Thu, Oct 8: still to come\nThu, Oct 1: 4 listens');
	});

	it('lists the artists of the week', () => {
		render(WeekGlance, { week: week(), artistHref });
		expect(screen.getByText(/1 heard for the first time/)).toBeInTheDocument();
		const ärzte = screen.getByRole('link', { name: /Die Ärzte/ });
		expect(ärzte).toHaveAttribute('href', '/artist/1');
		expect(ärzte).toHaveTextContent('2 last week');
		expect(screen.getByRole('link', { name: /Björk/ })).toHaveTextContent('new');
	});

	it('compares in every direction', () => {
		const { rerender } = render(WeekGlance, {
			week: week({ day: '2026-10-11', last_week_so_far: 20 }),
			artistHref
		});
		expect(screen.getByText('5 fewer than last week')).toBeInTheDocument();
		rerender({ week: week({ last_week_so_far: 15 }), artistHref });
		expect(screen.getByText('as many as last week by now')).toBeInTheDocument();
		rerender({ week: week({ last_week_so_far: 0 }), artistHref });
		expect(screen.getByText('none last week by now')).toBeInTheDocument();
	});

	it('handles a quiet week and short streaks', () => {
		const { rerender } = render(WeekGlance, {
			week: week({
				artists: [],
				new_artists: 0,
				streak: {
					current: 0,
					current_from: null,
					longest: 1,
					longest_from: '2012-01-01',
					longest_to: '2012-01-01'
				}
			}),
			artistHref
		});
		expect(screen.getByText('No listens yet this week.')).toBeInTheDocument();
		expect(screen.getByText('no listens yesterday or today')).toBeInTheDocument();
		expect(screen.getByText('on Jan 1, 2012')).toBeInTheDocument();
		rerender({ week: week({ streak: null }), artistHref });
		expect(screen.queryByText('Streak')).toBeNull();
	});
});

describe('ListeningClock', () => {
	// Most on Fridays (index 4) at 21:00.
	const weekdays = Array.from({ length: 7 }, (_, day) =>
		Array.from({ length: 24 }, (_, hour) =>
			hour === 21 ? (day === 4 ? 30 : 5) : day === 0 ? 1 : 0
		)
	);
	const clock: Clock = { listens: 30 + 6 * 5 + 23, weekdays };

	it('names the busiest day and hour', () => {
		render(ListeningClock, { clock });
		expect(screen.getByText('By weekday and hour, in Europe/Berlin')).toBeInTheDocument();
		expect(screen.getByText('Friday')).toBeInTheDocument();
		expect(screen.getByText(/^9\s?–\s?10\sPM$/)).toBeInTheDocument();
		expect(screen.getByRole('img')).toHaveAccessibleName(
			/^Listens by weekday and hour; most on Friday and at 9\s?–\s?10\sPM$/
		);
		expect(screen.getByTitle(/^Friday, 9\s?–\s?10\sPM: 30 listens$/)).toHaveClass('bg-yellow-500');
		expect(screen.getByTitle(/^Monday, 12\s?–\s?1\sAM: 1 listen$/)).toHaveClass('bg-yellow-100');
		expect(screen.getByTitle(/^Tuesday, 12\s?–\s?1\sAM: 0 listens$/)).toHaveClass(
			'bg-stone-200/60'
		);
	});

	it('starts the week with the viewer', () => {
		useTimeSettings({ time_zone: 'America/New_York', week_start: 7 } as never);
		const { container } = render(ListeningClock, { clock });
		expect(screen.getByText('By weekday and hour, in America/New York')).toBeInTheDocument();
		const rows = [...container.querySelectorAll('span.pr-1\\.5')].map((s) => s.textContent);
		expect(rows).toEqual(['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']);
	});
});

describe('MonthCurve', () => {
	const month = (m: string, listens: number) => ({ month: m, listens });

	it('draws nothing without months', () => {
		const { container } = render(MonthCurve, { months: [], phases: [] });
		expect(container.querySelector('svg')).toBeNull();
	});

	it('draws a bar per month with breaks for silent years', () => {
		layOut(600);
		const months = [
			month('2009-11', 10),
			month('2009-12', 0),
			month('2010-01', 40),
			month('2014-06', 20)
		];
		const { container } = render(MonthCurve, {
			months,
			phases: [{ from: '2010-01', to: '2010-01', listens: 40 }]
		});
		expect(screen.getByRole('img')).toHaveAccessibleName(
			'Listens per month from Nov 2009 to Jun 2014'
		);
		expect(screen.getByText('Dec 2009: 0 listens')).toBeInTheDocument();
		expect(screen.getByText('No listens between Jan 2010 and Jun 2014')).toBeInTheDocument();
		expect(container.querySelectorAll('rect.fill-yellow-500')).toHaveLength(1);
		expect(container.querySelectorAll('rect.fill-yellow-300')).toHaveLength(2);
		const labels = [...container.querySelectorAll('div[aria-hidden] span')].map(
			(s) => s.textContent
		);
		expect(labels).toEqual(['2009', '2010', '2014']);
	});

	it('thins out year labels when narrow', () => {
		layOut(300);
		const months = Array.from({ length: 20 * 12 }, (_, i) =>
			month(`${2000 + Math.floor(i / 12)}-${String((i % 12) + 1).padStart(2, '0')}`, i)
		);
		const { container } = render(MonthCurve, { months, phases: [] });
		const labels = [...container.querySelectorAll('div[aria-hidden] span')].map(
			(s) => s.textContent
		);
		expect(labels.length).toBeLessThan(10);
		expect(labels[0]).toBe('2000');
	});
});

describe('ArtistYears', () => {
	const years: YearTop[] = [
		{ year: 2010, listens: 100, artists: [artist(1, 'Die Ärzte', 50), artist(2, 'Björk', 20)] },
		{ year: 2011, listens: 80, artists: [artist(2, 'Björk', 30), artist(1, 'Die Ärzte', 10)] },
		{ year: 2014, listens: 60, artists: [artist(1, 'Die Ärzte', 40), artist(3, 'Tocotronic', 5)] }
	];

	it('lays out a column per year and colors those who stay', () => {
		layOut(800);
		visit('/u/fiona?source=Navidrome');
		const { container } = render(ArtistYears, { years, base: '/u/fiona', selected: 2011 });
		const legend = screen.getByText('Longest at the top:').parentElement!;
		const legendLinks = within(legend).getAllByRole('link');
		expect(legendLinks.map((a) => a.textContent?.trim())).toEqual(['Die Ärzte', 'Björk']);
		expect(legendLinks[0]).toHaveAttribute('href', '/u/fiona/artist/1-die-arzte?source=Navidrome');
		const column = screen.getByRole('list', { name: 'Top artists of 2011' });
		expect(within(column).getAllByRole('link')[0]).toHaveAccessibleName(
			'Björk, number 1 in 2011, 30 listens'
		);
		expect(screen.getByRole('link', { name: '2014' })).toHaveAttribute(
			'href',
			'?year=2014&source=Navidrome'
		);
		expect(screen.getByText('No listens from 2012 to 2013')).toBeInTheDocument();
		// Two artists move from 2010 to 2011; the break leaves 2014 unjoined.
		expect(container.querySelectorAll('path')).toHaveLength(2);
		expect(container.querySelector('.bg-yellow-100')).not.toBeNull();
	});

	it('highlights an artist under the pointer', async () => {
		layOut(800);
		visit('/u/fiona');
		const { container } = render(ArtistYears, { years, base: '/u/fiona' });
		const tocotronic = screen.getByRole('link', { name: /Tocotronic, number 2 in 2014/ });
		await fireEvent.pointerEnter(tocotronic);
		expect(container.querySelector('.z-10')).toHaveTextContent('#2 in 2014 · 5 listens · 8%');
		expect(screen.getByRole('link', { name: /Die Ärzte, number 1 in 2014/ })).toHaveClass(
			'opacity-30'
		);
		await fireEvent.pointerLeave(tocotronic);
		expect(container.querySelector('.z-10')).toBeNull();
		const legend = screen.getAllByRole('link', { name: 'Björk' })[0];
		await fireEvent.focus(legend);
		expect(container.querySelectorAll('path')).toHaveLength(3);
		await fireEvent.blur(legend);
		expect(container.querySelectorAll('path')).toHaveLength(2);
	});

	it('scrolls when the columns do not fit', () => {
		layOut(200);
		visit('/u/fiona');
		const many = Array.from({ length: 10 }, (_, i) => ({
			year: 2000 + i,
			listens: 10,
			artists: [artist(1, 'Die Ärzte', 10)]
		}));
		render(ArtistYears, { years: many, base: '/u/fiona', selected: 2001 });
		const region = screen.getByRole('region', { name: 'Top artists of each year' });
		expect((region.firstElementChild as HTMLElement).style.width).toBe(`${10 * 112 + 9 * 20}px`);
	});
});
