import { redirect } from '@sveltejs/kit';
import { getJson, type ApiToken } from '#lib/api.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ parent, fetch }) => {
	const { me } = await parent();
	if (!me) redirect(307, '/login?next=/settings');
	return { me, tokens: await getJson<ApiToken[]>(fetch, '/api/me/tokens') };
};
