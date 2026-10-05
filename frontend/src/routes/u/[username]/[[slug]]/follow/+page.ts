import { error, redirect } from '@sveltejs/kit';
import { ApiError, getJson, profileApi, type FollowInfo } from '#lib/api.ts';
import type { PageLoad } from './$types';

// The door of a profile for followers: who may not see it yet can ask here.
export const load: PageLoad = async ({ params, url, fetch, parent }) => {
	const { me } = await parent();
	if (!me) redirect(307, `/login?next=${encodeURIComponent(url.pathname)}`);
	try {
		const api = profileApi(params.username, params.slug ?? 'default');
		return { info: await getJson<FollowInfo>(fetch, `${api}/follow`) };
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) error(404, 'There is no such profile.');
		throw e;
	}
};
