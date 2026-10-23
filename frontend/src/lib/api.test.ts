import { describe, expect, it, vi } from 'vitest';
import {
	ApiError,
	entityPath,
	entitySegment,
	errorMessage,
	followRename,
	getJson,
	nameSlug,
	profileApi,
	profilePath,
	sendJson
} from '#lib/api.ts';

const json = (body: unknown, status = 200) =>
	new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

describe('getJson', () => {
	it('adds the parameters that are set', async () => {
		const fetch = vi.fn(async () => json({ ok: true }));
		expect(
			await getJson(fetch, '/api/x', { year: 2012, from: null, to: undefined, q: 'a b' })
		).toEqual({ ok: true });
		expect(fetch).toHaveBeenCalledWith('/api/x?year=2012&q=a+b');
		await getJson(fetch, '/api/y');
		expect(fetch).toHaveBeenLastCalledWith('/api/y');
	});

	it('throws the error text with the status', async () => {
		const fetch = vi.fn(async () => new Response('profile not found', { status: 404 }));
		const error = getJson(fetch, '/api/x');
		await expect(error).rejects.toBeInstanceOf(ApiError);
		await expect(error).rejects.toMatchObject({ status: 404, message: 'profile not found' });
	});

	it('falls back to the status text', async () => {
		const fetch = vi.fn(async () => new Response('', { status: 502, statusText: 'Bad Gateway' }));
		await expect(getJson(fetch, '/api/x')).rejects.toThrow('Bad Gateway');
	});
});

describe('sendJson', () => {
	it('sends a JSON body', async () => {
		const fetch = vi.fn(async () => json({ id: 3 }));
		expect(await sendJson(fetch, 'POST', '/api/tokens', { label: 'Navidrome' })).toEqual({ id: 3 });
		expect(fetch).toHaveBeenCalledWith('/api/tokens', {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: '{"label":"Navidrome"}'
		});
	});

	it('sends nothing without a body and gives null for 204', async () => {
		const fetch = vi.fn(async () => new Response(null, { status: 204 }));
		expect(await sendJson(fetch, 'DELETE', '/api/tokens/3')).toBeNull();
		expect(fetch).toHaveBeenCalledWith('/api/tokens/3', {
			method: 'DELETE',
			headers: {},
			body: undefined
		});
	});

	it('throws on errors', async () => {
		const fetch = vi.fn(async () => new Response('wrong password', { status: 403 }));
		await expect(sendJson(fetch, 'PUT', '/api/me/password', {})).rejects.toMatchObject({
			status: 403,
			message: 'wrong password'
		});
	});
});

describe('errorMessage', () => {
	it('capitalizes the message of anything thrown', () => {
		expect(errorMessage(new Error('no such profile'))).toBe('No such profile');
		expect(errorMessage('offline')).toBe('Offline');
	});
});

describe('paths', () => {
	it('builds API and page paths', () => {
		expect(profileApi('fiona', 'default')).toBe('/api/profiles/fiona/default');
		expect(profileApi('a/b', 'c d')).toBe('/api/profiles/a%2Fb/c%20d');
		expect(profilePath('fiona')).toBe('/u/fiona');
		expect(profilePath('fiona', 'default')).toBe('/u/fiona');
		expect(profilePath('fiona', 'work')).toBe('/u/fiona/work');
	});

	it('makes names into slugs', () => {
		expect(nameSlug('Die Ärzte')).toBe('die-arzte');
		expect(nameSlug('Jóga')).toBe('joga');
		expect(nameSlug('Straße')).toBe('strasse');
		expect(nameSlug('  ...Baby One More Time!  ')).toBe('baby-one-more-time');
		expect(nameSlug('!!!')).toBe('');
		expect(nameSlug('x'.repeat(80))).toHaveLength(60);
	});

	it('puts the id first', () => {
		expect(entitySegment(12, 'Die Ärzte')).toBe('12-die-arzte');
		expect(entitySegment(12, '???')).toBe('12');
		expect(entityPath('/u/fiona', 'artist', 12, 'Die Ärzte')).toBe('/u/fiona/artist/12-die-arzte');
	});
});

describe('followRename', () => {
	it('redirects to the new name', async () => {
		const fetch = vi.fn(async () => json({ username: 'fi' }));
		const error = await followRename(fetch, 'fiona', 'default', {
			pathname: '/u/fiona/artist/12',
			search: '?year=2012'
		}).catch((e) => e);
		expect(fetch).toHaveBeenCalledWith('/api/renamed/fiona/default');
		expect(error).toMatchObject({ status: 308, location: '/u/fi/artist/12?year=2012' });
	});

	it('returns when nobody was renamed', async () => {
		const fetch = vi.fn(async () => new Response('', { status: 404 }));
		await expect(
			followRename(fetch, 'nobody', 'default', { pathname: '/u/nobody', search: '' })
		).resolves.toBeUndefined();
	});
});
