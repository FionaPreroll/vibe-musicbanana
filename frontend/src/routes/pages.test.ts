// The small pages: the header around every page, the start page, login and errors.

import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { admin, me } from '../test/fixtures.ts';
import { fakeApi } from '../test/fetch.ts';
import { goto, invalidateAll, page, visit } from '../test/app.svelte.ts';
import ErrorPage from './+error.svelte';
import Layout from './+layout.svelte';
import Home from './+page.svelte';
import Login from './login/+page.svelte';

vi.mock('$app/state', () => import('../test/app.svelte.ts'));
vi.mock('$app/navigation', () => import('../test/app.svelte.ts'));

const children = createRawSnippet(() => ({ render: () => '<p>The page</p>' }));

beforeEach(() => {
	visit('/');
	page.params = {};
	page.route = { id: '/' };
	goto.mockClear();
	invalidateAll.mockClear();
	vi.stubGlobal('matchMedia', () => ({
		matches: false,
		addEventListener() {},
		removeEventListener() {}
	}));
});
afterEach(() => vi.unstubAllGlobals());

describe('layout', () => {
	it('offers guests the login, back to where they are', () => {
		visit('/u/fiona?year=2012');
		render(Layout, { data: { me: null }, children });
		expect(screen.getByText('The page')).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'Log in' })).toHaveAttribute(
			'href',
			'/login?next=%2Fu%2Ffiona%3Fyear%3D2012'
		);
		expect(screen.queryByRole('search')).toBeNull();
	});

	it('does not send the login page back to itself', () => {
		visit('/login?next=/status');
		render(Layout, { data: { me: null }, children });
		expect(screen.getByRole('link', { name: 'Log in' })).toHaveAttribute('href', '/login');
	});

	it('links the account, and the admin pages for admins', () => {
		const { unmount } = render(Layout, { data: { me }, children });
		expect(screen.getByRole('link', { name: 'fiona' })).toHaveAttribute('href', '/u/fiona');
		expect(screen.queryByRole('link', { name: 'Status' })).toBeNull();
		unmount();
		render(Layout, { data: { me: admin }, children });
		expect(screen.getByRole('link', { name: 'Status' })).toHaveAttribute('href', '/status');
		expect(screen.getByRole('link', { name: 'Merges' })).toHaveAttribute('href', '/merges');
	});

	it('opens and closes the menu on phones', async () => {
		render(Layout, { data: { me }, children });
		const button = screen.getByRole('button', { name: 'Menu' });
		await userEvent.click(button);
		expect(button).toHaveAttribute('aria-expanded', 'true');
		await userEvent.keyboard('{Escape}');
		expect(button).toHaveAttribute('aria-expanded', 'false');
		await userEvent.click(button);
		await userEvent.click(screen.getByText('The page'));
		expect(button).toHaveAttribute('aria-expanded', 'false');
	});

	it('logs out', async () => {
		const fetch = fakeApi({ 'DELETE /api/session': undefined });
		vi.stubGlobal('fetch', fetch);
		render(Layout, { data: { me }, children });
		await userEvent.click(screen.getByRole('button', { name: 'Log out' }));
		expect(fetch).toHaveBeenCalledWith(
			'/api/session',
			expect.objectContaining({ method: 'DELETE' })
		);
		expect(goto).toHaveBeenCalledWith('/', { invalidateAll: true });
	});

	it('searches the profile shown, with its source, and jumps there with /', async () => {
		visit('/u/fiona/work?source=Navidrome');
		page.params = { username: 'fiona', slug: 'work' };
		render(Layout, { data: { me: null }, children });
		const search = screen.getByRole('search');
		expect(search).toHaveAttribute('action', '/u/fiona/work/search');
		expect(search.querySelector('input[type=hidden]')).toHaveValue('Navidrome');
		const field = within(search).getByRole('searchbox');
		expect(field).toHaveAttribute('placeholder', 'Search fiona');
		await userEvent.keyboard('/');
		expect(field).toHaveFocus();
		// In a field, / is just a slash.
		await userEvent.keyboard('a/');
		expect(field).toHaveValue('a/');
	});

	it('leaves / alone without a search, or with a modifier', async () => {
		render(Layout, { data: { me: null }, children });
		await userEvent.keyboard('/');
		page.params = { username: 'fiona' };
		await userEvent.keyboard('{Control>}/{/Control}');
		expect(screen.getByRole('searchbox')).not.toHaveFocus();
	});

	it('leaves the search to the search page', () => {
		page.params = { username: 'fiona' };
		page.route = { id: '/u/[username]/[[slug]]/search' };
		render(Layout, { data: { me: null }, children });
		expect(screen.queryByRole('search')).toBeNull();
	});

	it('unfolds the banana', async () => {
		render(Layout, { data: { me: null }, children });
		const banana = screen.getByRole('button', { name: '🍌' });
		await userEvent.click(banana);
		expect(banana).toHaveAttribute('aria-expanded', 'true');
		expect(screen.getByText(/musicbanana already lives/)).toBeInTheDocument();
	});
});

describe('error page', () => {
	it('shows the status and message', () => {
		page.status = 404;
		page.error = { message: 'There is no such profile.' };
		render(ErrorPage);
		expect(screen.getByText('404')).toBeInTheDocument();
		expect(screen.getByText('There is no such profile.')).toBeInTheDocument();
	});
});

describe('start page', () => {
	const profile = (username: string, slug: string, visibility = 'public') => ({
		username,
		slug,
		name: slug === 'default' ? 'Default' : 'Work',
		visibility,
		listens: 1234
	});

	it('lists the profiles', () => {
		render(Home, {
			data: {
				me: null,
				following: [],
				profiles: [profile('fiona', 'default'), profile('fiona', 'work', 'private')]
			}
		} as never);
		expect(screen.getByRole('heading', { name: 'Profiles' })).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'fiona / Work' })).toHaveAttribute(
			'href',
			'/u/fiona/work'
		);
		expect(screen.getByText('Private')).toBeInTheDocument();
		expect(screen.getAllByText('1,234 listens')).toHaveLength(2);
	});

	it('lists whom the viewer follows first', () => {
		render(Home, {
			data: {
				me,
				profiles: [],
				following: [
					{ ...profile('bela', 'default'), state: 'following', since: '' },
					{ ...profile('farin', 'work', 'followers'), state: 'requested', since: '' }
				]
			}
		} as never);
		expect(screen.getByRole('heading', { name: 'You follow' })).toBeInTheDocument();
		expect(screen.getByRole('heading', { name: 'All profiles' })).toBeInTheDocument();
		expect(screen.getByText('No profiles to show yet.')).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'bela' })).toHaveAttribute('href', '/u/bela');
		expect(screen.getByRole('link', { name: 'farin / Work' })).toHaveAttribute(
			'href',
			'/u/farin/work/follow'
		);
		expect(screen.getByText('Asked, no answer yet')).toBeInTheDocument();
	});
});

describe('login', () => {
	async function logIn() {
		await userEvent.type(screen.getByLabelText('User name or email'), 'fiona');
		await userEvent.type(screen.getByLabelText('Password'), 'banana');
		await userEvent.click(screen.getByRole('button', { name: 'Log in' }));
	}

	it('logs in and goes to the profile', async () => {
		const fetch = fakeApi({
			'POST /api/session': (init: RequestInit) => {
				expect(JSON.parse(String(init.body))).toEqual({ login: 'fiona', password: 'banana' });
				return { username: 'fiona' };
			}
		});
		vi.stubGlobal('fetch', fetch);
		render(Login);
		await logIn();
		expect(invalidateAll).toHaveBeenCalled();
		expect(goto).toHaveBeenCalledWith('/u/fiona');
	});

	it('goes back to where it started, on this site only', async () => {
		vi.stubGlobal('fetch', fakeApi({ 'POST /api/session': { username: 'fiona' } }));
		visit('/login?next=/settings');
		const { unmount } = render(Login);
		await logIn();
		expect(goto).toHaveBeenLastCalledWith('/settings');
		unmount();
		visit('/login?next=//evil.example');
		render(Login);
		await logIn();
		expect(goto).toHaveBeenLastCalledWith('/u/fiona');
	});

	it('shows what went wrong', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({ 'POST /api/session': new Response('wrong password', { status: 401 }) })
		);
		render(Login);
		await logIn();
		expect(screen.getByRole('alert')).toHaveTextContent('Wrong password');
		expect(screen.getByRole('button', { name: 'Log in' })).toBeEnabled();
		expect(goto).not.toHaveBeenCalled();
	});
});
