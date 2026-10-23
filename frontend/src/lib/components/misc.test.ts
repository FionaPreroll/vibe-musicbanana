import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fakeApi } from '../../test/fetch.ts';
import BananaEgg from './BananaEgg.svelte';
import CatalogPicker from './CatalogPicker.svelte';
import ThemeToggle from './ThemeToggle.svelte';

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('BananaEgg', () => {
	it('tells the ages of musicbanana and closes with Escape', async () => {
		vi.useFakeTimers({ now: new Date('2026-10-23T12:00:00Z'), toFake: ['Date'] });
		const { rerender } = render(BananaEgg, { open: false });
		expect(screen.queryByText(/musicbanana already lives/)).toBeNull();
		await rerender({ open: true });
		expect(screen.getByText('musicbanana already lives 19 years, 2 months')).toHaveAttribute(
			'title',
			'since 2007-08-12T14:28:00.000Z'
		);
		expect(screen.getByText('has been a happy python for 16 years, 6 months')).toBeInTheDocument();
		expect(screen.getByText('has been a happy ElePHPant for 6 years, 1 month')).toBeInTheDocument();
		expect(screen.getByText('has been a happy Rustacean for 2 weeks, 5 days')).toBeInTheDocument();
		await userEvent.keyboard('{Escape}');
		await vi.waitFor(() => expect(screen.queryByText(/Rustacean/)).toBeNull());
	});
});

describe('ThemeToggle', () => {
	let systemDark = false;
	const listeners: (() => void)[] = [];

	beforeEach(() => {
		systemDark = false;
		listeners.length = 0;
		localStorage.clear();
		delete document.documentElement.dataset.theme;
		vi.stubGlobal('matchMedia', () => ({
			get matches() {
				return systemDark;
			},
			addEventListener: (_: string, f: () => void) => listeners.push(f),
			removeEventListener: vi.fn()
		}));
	});

	it('switches and remembers the pick', async () => {
		render(ThemeToggle);
		const button = screen.getByRole('button', { name: 'Dark mode' });
		expect(button).toHaveAttribute('aria-pressed', 'false');
		await userEvent.click(button);
		expect(button).toHaveAttribute('aria-pressed', 'true');
		expect(button).toHaveAttribute('title', 'Switch to light mode');
		expect(document.documentElement.dataset.theme).toBe('dark');
		expect(localStorage.getItem('theme')).toBe('dark');
		// Back to what the system shows: follow the system again.
		await userEvent.click(button);
		expect(document.documentElement.dataset.theme).toBe('light');
		expect(localStorage.getItem('theme')).toBeNull();
	});

	it('follows the system until something is picked', async () => {
		document.documentElement.dataset.theme = 'dark';
		systemDark = true;
		render(ThemeToggle);
		const button = screen.getByRole('button', { name: 'Dark mode' });
		expect(button).toHaveAttribute('aria-pressed', 'true');
		systemDark = false;
		listeners.forEach((f) => f());
		await vi.waitFor(() => expect(button).toHaveAttribute('aria-pressed', 'false'));
		localStorage.setItem('theme', 'light');
		systemDark = true;
		listeners.forEach((f) => f());
		expect(document.documentElement.dataset.theme).toBe('light');
	});

	it('works without storage', async () => {
		vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
			throw new Error('blocked');
		});
		vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
			throw new Error('blocked');
		});
		render(ThemeToggle);
		listeners.forEach((f) => f());
		await userEvent.click(screen.getByRole('button', { name: 'Dark mode' }));
		expect(document.documentElement.dataset.theme).toBe('dark');
		vi.restoreAllMocks();
	});
});

describe('CatalogPicker', () => {
	const hits = [
		{ id: 1, name: 'Die Ärzte', artist: null, listens: 500 },
		{ id: 2, name: 'Die Aerzte', artist: null, listens: 1 }
	];

	it('finds entries as one types and picks one', async () => {
		const fetch = fakeApi({ 'GET /api/admin/catalog/artist': hits });
		vi.stubGlobal('fetch', fetch);
		const { component } = render(CatalogPicker, { kind: 'artist', label: 'Merge', picked: null });
		const input = screen.getByRole('searchbox', { name: 'Merge' });
		await userEvent.type(input, 'ärzte');
		await userEvent.click(await screen.findByRole('button', { name: /Die Aerzte/ }));
		expect(fetch).toHaveBeenCalledTimes(1);
		expect(fetch).toHaveBeenCalledWith('/api/admin/catalog/artist?q=%C3%A4rzte');
		expect(screen.getByText('· #2 · 1 listen')).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Pick another' }));
		expect(screen.getByRole('searchbox', { name: 'Merge' })).toHaveValue('');
		expect(component).toBeTruthy();
	});

	it('shows the artist of releases and errors', async () => {
		let fail = false;
		vi.stubGlobal(
			'fetch',
			fakeApi({
				'GET /api/admin/catalog/release': () =>
					fail
						? new Response('not an admin', { status: 403 })
						: [{ id: 3, name: 'Die Bestie in Menschengestalt', artist: 'Die Ärzte', listens: 2 }]
			})
		);
		render(CatalogPicker, { kind: 'release', label: 'Into', picked: null });
		const input = screen.getByRole('searchbox', { name: 'Into' });
		await userEvent.type(input, 'bestie');
		expect(await screen.findByText('· Die Ärzte')).toBeInTheDocument();
		fail = true;
		await userEvent.type(input, 'x');
		expect(await screen.findByText('Not an admin')).toBeInTheDocument();
		await userEvent.clear(input);
		await userEvent.tab();
		await vi.waitFor(() => expect(screen.queryByText('Not an admin')).toBeNull());
	});
});
