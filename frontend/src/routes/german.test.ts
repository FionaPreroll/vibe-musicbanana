// Pages in German, and picking the language in the settings.

import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Week } from '#lib/api.ts';
import WeekGlance from '#lib/components/WeekGlance.svelte';
import Fill from '#lib/i18n/Fill.svelte';
import { language } from '#lib/i18n/index.ts';
import { useLanguage } from '#lib/i18n/locale.svelte.ts';
import { invalidateAll } from '../test/app.svelte.ts';
import { fakeApi } from '../test/fetch.ts';
import * as fixtures from '../test/fixtures.ts';
import SettingsPage from './settings/+page.svelte';
import YearPage from './u/[username]/[[slug]]/year/[year=year]/+page.svelte';
import { load as layoutLoad } from './+layout.ts';

vi.mock('$app/state', () => import('../test/app.svelte.ts'));
vi.mock('$app/navigation', () => import('../test/app.svelte.ts'));

beforeEach(() => {
	useLanguage('de');
	vi.useFakeTimers({ now: new Date('2026-10-07T10:00:00Z'), toFake: ['Date'] });
});
afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
	useLanguage(null);
});

describe('Fill', () => {
	const link = createRawSnippet(() => ({ render: () => '<a href="/x">Die Ärzte</a>' }));

	it('puts snippets into the holes of a text', () => {
		const { container } = render(Fill, { text: 'von {artist}, sonst {nothing}', artist: link });
		expect(container).toHaveTextContent('von Die Ärzte, sonst {nothing}');
		expect(screen.getByRole('link', { name: 'Die Ärzte' })).toHaveAttribute('href', '/x');
	});
});

describe('week at a glance', () => {
	const week: Week = {
		from: '2026-10-05',
		to: '2026-10-11',
		day: '2026-10-07',
		days: ['05', '06', '07', '08', '09', '10', '11'].map((d, i) => ({
			date: `2026-10-${d}`,
			listens: i < 3 ? [10, 0, 1500][i] : 0,
			last_week: 4
		})),
		listens: 1510,
		last_week_so_far: 12,
		last_week: 28,
		artists: [
			{ id: 1, name: 'Die Ärzte', listens: 1200, last_week: 2, new: false },
			{ id: 2, name: 'Björk', listens: 310, last_week: 0, new: true }
		],
		new_artists: 1,
		streak: {
			current: 1,
			current_from: '2026-10-07',
			longest: 40,
			longest_from: '2012-01-01',
			longest_to: '2012-02-09'
		}
	};

	it('reads in German, with German numbers and dates', () => {
		render(WeekGlance, { week, artistHref: (id: number) => `/artist/${id}` });
		expect(screen.getByRole('heading', { name: 'Diese Woche' })).toBeInTheDocument();
		expect(screen.getByText('Mo., 5. Okt. bis So., 11. Okt.')).toBeInTheDocument();
		expect(screen.getByText('1.510')).toBeInTheDocument();
		expect(screen.getByText('1.498 mehr als letzte Woche um diese Zeit')).toBeInTheDocument();
		expect(screen.getByText('Tage mit Musik').nextElementSibling).toHaveTextContent('2 von 3');
		expect(screen.getByText('1 Tag')).toBeInTheDocument();
		expect(screen.getByText('am Stück seit dem 07.10.2026')).toBeInTheDocument();
		expect(screen.getByText('vom 01.01.2012 bis 09.02.2012')).toBeInTheDocument();
		expect(screen.getByText('neu')).toBeInTheDocument();
		expect(screen.getByText('· 2 in der Vorwoche')).toBeInTheDocument();
		expect(screen.getAllByRole('listitem')[3]).toHaveAttribute(
			'title',
			'Do., 8. Okt.: kommt noch\nDo., 1. Okt.: 4 Wiedergaben'
		);
	});
});

describe('year in review', () => {
	it('reads in German, the list of artists too', () => {
		render(YearPage, {
			params: {} as never,
			data: {
				me: null,
				base: '/u/fiona',
				source: null,
				sources: [],
				overview: fixtures.overview,
				review: fixtures.review,
				artists: fixtures.artists,
				releases: fixtures.releases,
				recordings: fixtures.recordings
			} as never
		});
		expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Jahresrückblick 2010');
		expect(screen.getByText('50 % mehr als 2009')).toBeInTheDocument();
		expect(screen.getByText('Juli')).toBeInTheDocument();
		expect(screen.getByText('3 Std. 56 Min.')).toBeInTheDocument();
		expect(screen.getByText('zurück')).toBeInTheDocument();
		const session = screen.getByText(/Eine Session läuft weiter/);
		expect(session).toHaveTextContent(
			'Vor allem Die Ärzte (40), Björk (15) und Tocotronic (6). Eine Session läuft weiter'
		);
		expect(within(session).getByRole('link', { name: 'Björk' })).toHaveAttribute(
			'href',
			expect.stringContaining('/u/fiona/artist/')
		);
	});
});

describe('the language in the settings', () => {
	const data = (language: string | null) =>
		({
			me: { ...fixtures.me, language },
			profiles: [],
			tokens: [],
			connections: [],
			allowed: [],
			followers: []
		}) as never;

	it('switches at once and is saved for the account', async () => {
		const sent: unknown[] = [];
		vi.stubGlobal(
			'fetch',
			fakeApi({
				'PUT /api/me/language': (init: RequestInit) => {
					sent.push(JSON.parse(String(init.body)));
					return JSON.parse(String(init.body));
				}
			})
		);
		render(SettingsPage, { params: {} as never, data: data('de') });
		const picker = screen.getByRole('combobox', { name: 'Sprache' });
		expect(picker).toHaveValue('de');
		expect(within(picker).getByRole('option', { name: 'English' })).toHaveAttribute('lang', 'en');

		await userEvent.selectOptions(picker, 'en');
		expect(language()).toBe('en');
		expect(screen.getByRole('heading', { name: 'Settings', level: 1 })).toBeInTheDocument();
		expect(sent).toEqual([{ language: 'en' }]);
		expect(invalidateAll).toHaveBeenCalled();

		await userEvent.selectOptions(screen.getByRole('combobox', { name: 'Language' }), '');
		expect(sent.at(-1)).toEqual({ language: null });
	});

	it('shows in German why it could not be saved', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({
				'PUT /api/me/language': new Response('unknown language: xx', { status: 400 })
			})
		);
		render(SettingsPage, { params: {} as never, data: data(null) });
		await userEvent.selectOptions(screen.getByRole('combobox', { name: 'Sprache' }), 'de');
		expect(await screen.findByRole('alert')).toHaveTextContent('Unbekannte Sprache: xx');
	});
});

describe('the layout', () => {
	it('takes the language of the account', async () => {
		useLanguage(null);
		vi.stubGlobal('fetch', fakeApi({ 'GET /api/me': { ...fixtures.me, language: 'de' } }));
		await layoutLoad({ fetch } as never);
		expect(language()).toBe('de');
		expect(document.documentElement.lang).toBe('de');
	});
});
