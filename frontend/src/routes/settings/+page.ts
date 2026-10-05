import { redirect } from '@sveltejs/kit';
import { getJson, type ApiToken, type Connection, type Follower } from '#lib/api.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ parent, fetch }) => {
	const { me } = await parent();
	if (!me) redirect(307, '/login?next=/settings');
	const [tokens, connections, allowed, followers] = await Promise.all([
		getJson<ApiToken[]>(fetch, '/api/me/tokens'),
		getJson<Connection[]>(fetch, '/api/me/yourspotify'),
		// The YourSpotify addresses the server takes from here.
		getJson<string[]>(fetch, '/api/me/yourspotify/allowed'),
		getJson<Follower[]>(fetch, '/api/me/followers')
	]);
	return { me, tokens, connections, allowed, followers };
};
