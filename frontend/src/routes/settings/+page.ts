import { redirect } from '@sveltejs/kit';
import { getJson, type ApiToken, type Connection } from '#lib/api.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ parent, fetch }) => {
	const { me } = await parent();
	if (!me) redirect(307, '/login?next=/settings');
	const [tokens, connections] = await Promise.all([
		getJson<ApiToken[]>(fetch, '/api/me/tokens'),
		getJson<Connection[]>(fetch, '/api/me/yourspotify')
	]);
	return { me, tokens, connections };
};
