import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FollowInfo } from '#lib/api.ts';
import FollowButton from './FollowButton.svelte';

const info = (fields: Partial<FollowInfo> = {}): FollowInfo => ({
	username: 'bela',
	slug: 'default',
	name: 'Bela',
	visibility: 'public',
	own: false,
	state: 'none',
	...fields
});

afterEach(() => vi.unstubAllGlobals());

describe('FollowButton', () => {
	it('follows a public profile', async () => {
		const fetch = vi.fn(async () => Response.json(info({ state: 'following' })));
		vi.stubGlobal('fetch', fetch);
		const onchange = vi.fn();
		render(FollowButton, { info: info(), onchange });
		await userEvent.click(screen.getByRole('button', { name: 'Follow' }));
		expect(fetch).toHaveBeenCalledWith('/api/profiles/bela/default/follow', {
			method: 'PUT',
			headers: {},
			body: undefined
		});
		expect(await screen.findByText('Following')).toBeInTheDocument();
		expect(onchange).toHaveBeenCalledWith(expect.objectContaining({ state: 'following' }));
	});

	it('asks to follow a followers-only profile and takes it back', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(async (_: string, init: RequestInit) =>
				init.method === 'PUT'
					? Response.json(info({ visibility: 'followers', state: 'requested' }))
					: new Response(null, { status: 204 })
			)
		);
		render(FollowButton, { info: info({ visibility: 'followers' }) });
		await userEvent.click(screen.getByRole('button', { name: 'Ask to follow' }));
		expect(await screen.findByText('Asked bela to follow')).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Take back' }));
		expect(await screen.findByRole('button', { name: 'Ask to follow' })).toBeInTheDocument();
	});

	it('asks before unfollowing a followers-only profile', async () => {
		const fetch = vi.fn(async () => new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetch);
		const confirm = vi.fn(() => false);
		vi.stubGlobal('confirm', confirm);
		render(FollowButton, { info: info({ visibility: 'followers', state: 'following' }) });
		await userEvent.click(screen.getByRole('button', { name: 'Unfollow' }));
		expect(confirm).toHaveBeenCalledWith(
			"Stop following? You won't see this profile until bela says yes again."
		);
		expect(fetch).not.toHaveBeenCalled();
		confirm.mockReturnValue(true);
		await userEvent.click(screen.getByRole('button', { name: 'Unfollow' }));
		expect(await screen.findByRole('button', { name: 'Ask to follow' })).toBeInTheDocument();
	});

	it('shows errors', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => new Response('too many requests', { status: 429 }))
		);
		render(FollowButton, { info: info() });
		await userEvent.click(screen.getByRole('button', { name: 'Follow' }));
		expect(await screen.findByRole('alert')).toHaveTextContent('Too many requests');
		expect(screen.getByRole('button', { name: 'Follow' })).toBeEnabled();
	});
});
