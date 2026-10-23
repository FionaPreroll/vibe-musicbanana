// The settings page, one section after the other.

import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ApiToken, Connection, Follower, Me, OwnProfile } from '#lib/api.ts';
import { invalidateAll } from '../test/app.svelte.ts';
import { fakeApi } from '../test/fetch.ts';
import { me } from '../test/fixtures.ts';
import SettingsPage from './settings/+page.svelte';

vi.mock('$app/state', () => import('../test/app.svelte.ts'));
vi.mock('$app/navigation', () => import('../test/app.svelte.ts'));

const profiles: OwnProfile[] = [
	{ slug: 'default', name: 'Default', visibility: 'public', listens: 2048 },
	{ slug: 'work', name: 'Work', visibility: 'private', listens: 1 }
];
const tokens: ApiToken[] = [
	{
		id: 4,
		profile: 'default',
		label: 'Navidrome',
		created_at: '2026-10-01T00:00:00Z',
		last_used_at: '2026-10-07T09:55:00Z'
	},
	{
		id: 5,
		profile: 'work',
		label: 'Ultrasonic',
		created_at: '2026-10-01T00:00:00Z',
		last_used_at: null
	}
];
const connection = (fields: Partial<Connection> = {}): Connection => ({
	id: 3,
	username: 'fiona',
	profile: 'default',
	url: 'http://yourspotify:8080',
	created_at: '2026-10-01T00:00:00Z',
	started_at: '2026-10-07T09:00:00Z',
	finished_at: '2026-10-07T09:01:00Z',
	imported: 5000,
	error: null,
	refetch_requested_at: null,
	refetch_at: null,
	refetch_plays: 0,
	refetch_new: 0,
	refetch_left_out: 0,
	refetched_at: null,
	...fields
});
const followers: Follower[] = [
	{ profile: 'work', username: 'bela', state: 'requested', since: '2026-10-06T12:00:00Z' },
	{ profile: 'default', username: 'farin', state: 'following', since: '2026-10-05T12:00:00Z' }
];

const data = (fields: Record<string, unknown> = {}, account: Partial<Me> = {}) =>
	({
		me: { ...me, ...account },
		profiles,
		tokens,
		connections: [connection()],
		allowed: ['http://yourspotify:8080'],
		followers,
		...fields
	}) as never;

/** Records what each route was sent, and answers with `answers[route]` (or nothing). */
function api(answers: Record<string, unknown> = {}) {
	const sent: Record<string, unknown[]> = {};
	const routes = new Proxy(
		{},
		{
			has: () => true,
			get: (_, key: string) => (init: RequestInit) => {
				(sent[key] ??= []).push(
					init.body === undefined ? undefined : JSON.parse(String(init.body))
				);
				return answers[key];
			}
		}
	);
	vi.stubGlobal('fetch', fakeApi(routes));
	return sent;
}

const section = (name: string) =>
	screen.getByRole('heading', { name, level: 2 }).closest('section') as HTMLElement;

beforeEach(() => {
	invalidateAll.mockClear();
	vi.useFakeTimers({ now: new Date('2026-10-07T10:00:00Z'), toFake: ['Date'] });
});
afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('profiles', () => {
	it('lists them with their history and visibility', () => {
		api();
		render(SettingsPage, { params: {} as never, data: data() });
		const profilesSection = section('Profiles');
		expect(
			within(profilesSection).getByRole('link', { name: '/u/fiona/work' })
		).toBeInTheDocument();
		expect(within(profilesSection).getByText('2,048 listens')).toBeInTheDocument();
		expect(
			within(profilesSection).getAllByRole('link', { name: 'Edit listening history' })[1]
		).toHaveAttribute('href', '/settings/history/work');
		expect(screen.getByRole('combobox', { name: 'Who sees Work' })).toHaveValue('private');
	});

	it('renames one and changes who sees it', async () => {
		const sent = api();
		render(SettingsPage, { params: {} as never, data: data() });
		const name = screen.getByRole('textbox', { name: 'Name of work' });
		const rename = within(name.closest('form')!).getByRole('button', { name: 'Rename' });
		expect(rename).toBeDisabled();
		await userEvent.clear(name);
		await userEvent.type(name, 'Office');
		await userEvent.click(rename);
		expect(sent['PATCH /api/me/profiles/work']).toEqual([{ name: 'Office' }]);
		expect(invalidateAll).toHaveBeenCalled();
		await userEvent.selectOptions(
			screen.getByRole('combobox', { name: 'Who sees Work' }),
			'followers'
		);
		expect(sent['PATCH /api/me/profiles/work'][1]).toEqual({ visibility: 'followers' });
	});

	it('shows an error next to the profile', async () => {
		vi.stubGlobal('fetch', fakeApi({}));
		render(SettingsPage, { params: {} as never, data: data() });
		await userEvent.selectOptions(
			screen.getByRole('combobox', { name: 'Who sees Default' }),
			'private'
		);
		const item = screen.getByRole('combobox', { name: 'Who sees Default' }).closest('li')!;
		expect(within(item).getByRole('alert')).toHaveTextContent(/No route PATCH/);
	});

	it('creates a new one', async () => {
		const sent = api();
		render(SettingsPage, { params: {} as never, data: data() });
		const form = screen.getByRole('heading', { name: 'New profile' }).closest('form')!;
		await userEvent.type(within(form).getByLabelText('Address'), 'spotify');
		await userEvent.type(within(form).getByLabelText('Name'), 'Spotify');
		await userEvent.selectOptions(within(form).getByLabelText('Who sees it'), 'followers');
		expect(form).toHaveTextContent('The address makes /u/fiona/spotify.');
		await userEvent.click(within(form).getByRole('button', { name: 'Create profile' }));
		expect(sent['POST /api/me/profiles']).toEqual([
			{ slug: 'spotify', name: 'Spotify', visibility: 'followers' }
		]);
		expect(within(form).getByLabelText('Address')).toHaveValue('');
	});
});

describe('scrobble tokens', () => {
	it('lists them', () => {
		api();
		render(SettingsPage, { params: {} as never, data: data() });
		const rows = within(section('Scrobble tokens')).getAllByRole('row');
		expect(rows[1]).toHaveTextContent('NavidromedefaultOct 7, 2026, 11:55 AMRevoke');
		expect(rows[2]).toHaveTextContent('never');
	});

	it('says when there are none', () => {
		api();
		render(SettingsPage, { params: {} as never, data: data({ tokens: [] }) });
		expect(screen.getByText('No tokens yet.')).toBeInTheDocument();
	});

	it('makes one and shows it once', async () => {
		const sent = api({ 'POST /api/me/tokens': { token: 'secret-uuid' } });
		render(SettingsPage, { params: {} as never, data: data() });
		const tokensSection = section('Scrobble tokens');
		const form = within(tokensSection)
			.getByRole('button', { name: 'Create token' })
			.closest('form')!;
		await userEvent.selectOptions(within(form).getByLabelText('Profile'), 'work');
		await userEvent.type(within(form).getByLabelText('Label'), 'Supersonic');
		await userEvent.click(within(form).getByRole('button', { name: 'Create token' }));
		expect(sent['POST /api/me/tokens']).toEqual([{ profile: 'work', label: 'Supersonic' }]);
		const shown = within(tokensSection).getByRole('status');
		expect(shown).toHaveTextContent('The token for Supersonic.');
		const field = within(shown).getByRole('textbox');
		expect(field).toHaveValue('secret-uuid');
		await userEvent.click(field);
		await userEvent.click(within(shown).getByRole('button', { name: 'Done' }));
		expect(within(tokensSection).queryByRole('status')).toBeNull();
	});

	it('revokes one after asking', async () => {
		const sent = api();
		const confirm = vi.fn(() => false);
		vi.stubGlobal('confirm', confirm);
		render(SettingsPage, { params: {} as never, data: data() });
		const revoke = within(section('Scrobble tokens')).getAllByRole('button', { name: 'Revoke' })[0];
		await userEvent.click(revoke);
		expect(confirm).toHaveBeenCalledWith(
			`Revoke the token "Navidrome"? Its client can't scrobble any more.`
		);
		expect(sent['DELETE /api/me/tokens/4']).toBeUndefined();
		confirm.mockReturnValue(true);
		await userEvent.click(revoke);
		expect(sent['DELETE /api/me/tokens/4']).toHaveLength(1);
	});
});

describe('YourSpotify', () => {
	it('lists connections with their state', () => {
		api();
		render(SettingsPage, {
			params: {} as never,
			data: data({
				connections: [
					connection(),
					connection({
						id: 4,
						profile: 'work',
						error: 'bad token',
						refetch_requested_at: '2026-10-07T09:30:00Z'
					})
				]
			})
		});
		const spotify = section('Spotify via YourSpotify');
		expect(within(spotify).getAllByText('5,000 listens so far')).toHaveLength(2);
		expect(
			within(spotify).getByText('Up to date as of Oct 7, 2026, 11:01 AM.')
		).toBeInTheDocument();
		expect(within(spotify).getByText('Failed at Oct 7, 2026, 11:01 AM: bad token')).toHaveClass(
			'text-red-700'
		);
		expect(
			within(spotify).getByText('Fetches everything again within a minute.')
		).toBeInTheDocument();
		expect(within(spotify).getAllByRole('button', { name: 'Fetch all again' })[1]).toBeDisabled();
	});

	it('fetches all again and removes, after asking', async () => {
		const sent = api();
		const confirm = vi.fn(() => false);
		vi.stubGlobal('confirm', confirm);
		render(SettingsPage, { params: {} as never, data: data() });
		const spotify = section('Spotify via YourSpotify');
		await userEvent.click(within(spotify).getByRole('button', { name: 'Fetch all again' }));
		await userEvent.click(within(spotify).getByRole('button', { name: 'Remove' }));
		expect(confirm).toHaveBeenCalledTimes(2);
		expect(Object.keys(sent)).toEqual([]);
		confirm.mockReturnValue(true);
		await userEvent.click(within(spotify).getByRole('button', { name: 'Fetch all again' }));
		await userEvent.click(within(spotify).getByRole('button', { name: 'Remove' }));
		expect(Object.keys(sent)).toEqual([
			'POST /api/me/yourspotify/3/refetch',
			'DELETE /api/me/yourspotify/3'
		]);
	});

	it('connects a profile with the one allowed address', async () => {
		const sent = api();
		render(SettingsPage, { params: {} as never, data: data({ connections: [] }) });
		const spotify = section('Spotify via YourSpotify');
		expect(within(spotify).getByLabelText('YourSpotify API address')).toHaveValue(
			'http://yourspotify:8080'
		);
		await userEvent.selectOptions(within(spotify).getByLabelText('Profile'), 'work');
		await userEvent.type(within(spotify).getByLabelText('Public token'), 'public-token');
		await userEvent.click(within(spotify).getByRole('button', { name: 'Connect' }));
		expect(sent['POST /api/me/yourspotify']).toEqual([
			{ profile: 'work', url: 'http://yourspotify:8080', token: 'public-token' }
		]);
		expect(within(spotify).getByLabelText('Public token')).toHaveValue('');
	});

	it('shows why a connection failed', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({
				'POST /api/me/yourspotify': new Response('YourSpotify does not answer', { status: 502 })
			})
		);
		render(SettingsPage, {
			params: {} as never,
			data: data({ connections: [], allowed: ['http://a:8080', 'http://b:8080'] })
		});
		const spotify = section('Spotify via YourSpotify');
		expect(spotify).toHaveTextContent('http://a:8080 or http://b:8080');
		await userEvent.type(
			within(spotify).getByLabelText('YourSpotify API address'),
			'http://b:8080'
		);
		await userEvent.type(within(spotify).getByLabelText('Public token'), 'x');
		await userEvent.click(within(spotify).getByRole('button', { name: 'Connect' }));
		expect(within(spotify).getByRole('alert')).toHaveTextContent('YourSpotify does not answer');
		expect(within(spotify).getByLabelText('Public token')).toHaveValue('x');
	});

	it('explains when the server allows no addresses', () => {
		api();
		render(SettingsPage, { params: {} as never, data: data({ allowed: [] }) });
		expect(screen.getByText(/YOURSPOTIFY_ALLOWED_URLS/)).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Connect' })).toBeNull();
	});

	it('asks again while an import runs', async () => {
		vi.useFakeTimers({ now: new Date('2026-10-07T10:00:00Z') });
		api();
		const { unmount } = render(SettingsPage, {
			params: {} as never,
			data: data({ connections: [connection({ finished_at: null })] })
		});
		await vi.advanceTimersByTimeAsync(10_000);
		expect(invalidateAll).toHaveBeenCalledTimes(1);
		unmount();
		render(SettingsPage, { params: {} as never, data: data() });
		await vi.advanceTimersByTimeAsync(30_000);
		expect(invalidateAll).toHaveBeenCalledTimes(1);
	});
});

describe('followers', () => {
	it('answers requests and removes followers', async () => {
		const sent = api();
		const confirm = vi.fn(() => true);
		vi.stubGlobal('confirm', confirm);
		render(SettingsPage, { params: {} as never, data: data() });
		const list = section('Followers');
		expect(list).toHaveTextContent('bela asks to follow /u/fiona/work since Oct 6, 2026, 2:00 PM');
		expect(list).toHaveTextContent('farin follows /u/fiona as of Oct 5, 2026, 2:00 PM');
		await userEvent.click(within(list).getByRole('button', { name: 'Say yes' }));
		await userEvent.click(within(list).getByRole('button', { name: 'Say no' }));
		expect(confirm).toHaveBeenLastCalledWith('Say no to bela?');
		await userEvent.click(within(list).getByRole('button', { name: 'Remove' }));
		expect(confirm).toHaveBeenLastCalledWith('Remove farin?');
		expect(Object.keys(sent)).toEqual([
			'PUT /api/me/followers/work/bela',
			'DELETE /api/me/followers/work/bela',
			'DELETE /api/me/followers/default/farin'
		]);
	});

	it('says when nobody follows', () => {
		api();
		render(SettingsPage, { params: {} as never, data: data({ followers: [] }) });
		expect(screen.getByText('Nobody follows your profiles yet.')).toBeInTheDocument();
	});
});

describe('time', () => {
	it('saves a time zone and first weekday', async () => {
		const sent = api();
		render(SettingsPage, { params: {} as never, data: data() });
		const time = section('Time');
		const save = within(time).getByRole('button', { name: 'Save' });
		expect(save).toBeDisabled();
		const zone = within(time).getByLabelText('Time zone');
		expect(zone).toHaveValue('');
		expect(
			within(zone).getByRole('option', { name: "This browser's (Europe/Berlin)" })
		).toBeInTheDocument();
		await userEvent.selectOptions(zone, 'America/New_York');
		await userEvent.selectOptions(within(time).getByLabelText('Weeks start on'), 'Sunday');
		await userEvent.click(save);
		expect(sent['PUT /api/me/time']).toEqual([{ time_zone: 'America/New_York', week_start: 7 }]);
		expect(within(time).getByRole('status')).toHaveTextContent('Saved.');
	});

	it('keeps a saved zone this browser does not know, and can go back to the browser', async () => {
		const sent = api();
		render(SettingsPage, {
			params: {} as never,
			data: data({}, { time_zone: 'Antarctica/Troll_Old', week_start: 6 })
		});
		const time = section('Time');
		const zone = within(time).getByLabelText('Time zone');
		expect(zone).toHaveValue('Antarctica/Troll_Old');
		expect(within(time).getByLabelText('Weeks start on')).toHaveValue('6');
		await userEvent.selectOptions(zone, '');
		await userEvent.click(within(time).getByRole('button', { name: 'Save' }));
		expect(sent['PUT /api/me/time']).toEqual([{ time_zone: null, week_start: 6 }]);
	});
});

describe('streaks', () => {
	it('turns them off', async () => {
		const sent = api();
		render(SettingsPage, { params: {} as never, data: data() });
		const box = within(section('Streaks')).getByRole('checkbox');
		expect(box).toBeChecked();
		await userEvent.click(box);
		expect(sent['PUT /api/me/streaks']).toEqual([{ show_streaks: false }]);
	});
});

describe('account', () => {
	it('changes the user name', async () => {
		const sent = api();
		render(SettingsPage, { params: {} as never, data: data() });
		const names = section('User name');
		const field = within(names).getByRole('textbox', { name: 'User name' });
		const rename = within(names).getByRole('button', { name: 'Rename' });
		expect(rename).toBeDisabled();
		await userEvent.clear(field);
		await userEvent.type(field, 'fi');
		await userEvent.click(rename);
		expect(sent['PUT /api/me/username']).toEqual([{ username: 'fi' }]);
		// The page shows the new name once the data is loaded again.
		expect(within(names).getByRole('status')).toHaveTextContent(
			'Renamed. Links with fiona lead to /u/fiona.'
		);
	});

	it('shows why a name is taken', async () => {
		vi.stubGlobal(
			'fetch',
			fakeApi({ 'PUT /api/me/username': new Response('taken', { status: 409 }) })
		);
		render(SettingsPage, { params: {} as never, data: data() });
		const names = section('User name');
		const field = within(names).getByRole('textbox', { name: 'User name' });
		await userEvent.type(field, '2');
		await userEvent.click(within(names).getByRole('button', { name: 'Rename' }));
		expect(within(names).getByRole('alert')).toHaveTextContent('Taken');
		expect(within(names).queryByRole('status')).toBeNull();
	});

	it('changes the email address with the password', async () => {
		const sent = api();
		render(SettingsPage, { params: {} as never, data: data() });
		const mail = section('Email address');
		const address = within(mail).getByLabelText('New address');
		expect(address).toHaveValue('fiona@example.org');
		await userEvent.clear(address);
		await userEvent.type(address, 'fiona@example.com');
		await userEvent.type(within(mail).getByLabelText('Your password'), 'banana');
		await userEvent.click(within(mail).getByRole('button', { name: 'Change address' }));
		expect(sent['PUT /api/me/email']).toEqual([{ email: 'fiona@example.com', password: 'banana' }]);
		expect(within(mail).getByRole('status')).toHaveTextContent('Changed to fiona@example.org.');
	});

	it('changes the password when both new ones agree', async () => {
		const sent = api();
		render(SettingsPage, { params: {} as never, data: data() });
		const pw = section('Password');
		await userEvent.type(within(pw).getByLabelText('Current password'), 'banana');
		await userEvent.type(within(pw).getByLabelText('New password'), 'bananarama');
		await userEvent.type(within(pw).getByLabelText('New password again'), 'bananaram');
		await userEvent.click(within(pw).getByRole('button', { name: 'Change password' }));
		expect(within(pw).getByRole('alert')).toHaveTextContent('The new passwords differ.');
		expect(sent['PUT /api/me/password']).toBeUndefined();
		await userEvent.type(within(pw).getByLabelText('New password again'), 'a');
		await userEvent.click(within(pw).getByRole('button', { name: 'Change password' }));
		expect(sent['PUT /api/me/password']).toEqual([{ current: 'banana', new: 'bananarama' }]);
		expect(within(pw).getByRole('status')).toHaveTextContent('Changed.');
		expect(within(pw).getByLabelText('Current password')).toHaveValue('');
	});

	it('offers the data export', () => {
		api();
		render(SettingsPage, { params: {} as never, data: data() });
		expect(screen.getByRole('link', { name: 'Download your data' })).toHaveAttribute(
			'href',
			'/api/me/export'
		);
	});
});
