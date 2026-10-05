import { error, redirect } from '@sveltejs/kit';
import { ApiError, getJson, type Status } from '#lib/api.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ parent, fetch }) => {
	const { me } = await parent();
	if (!me) redirect(307, '/login?next=/status');
	try {
		return { status: await getJson<Status>(fetch, '/api/admin/status') };
	} catch (e) {
		if (e instanceof ApiError && e.status === 403) error(403, 'The status page is for admins.');
		throw e;
	}
};
