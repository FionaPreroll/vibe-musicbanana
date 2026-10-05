import { error, redirect } from '@sveltejs/kit';
import { ApiError, getJson, profileApi, type HistoryPage, type Source } from '#lib/api.ts';
import { filterQuery } from '#lib/history.ts';
import type { PageLoad } from './$types';

// The listens of one of the viewer's profiles, narrowed by the filter in the
// URL (?q=&from=&to=&source=, from and to as local date and time), or its trash
// (?trash).
export const load: PageLoad = async ({ params, url, fetch, parent }) => {
	const { me } = await parent();
	if (!me) redirect(307, `/login?next=${encodeURIComponent(url.pathname + url.search)}`);
	const profile = me.profiles.find((p) => p.slug === params.slug);
	if (!profile) error(404, 'You have no such profile.');

	const api = `/api/me/profiles/${encodeURIComponent(profile.slug)}`;
	const trash = url.searchParams.has('trash');
	const filter = {
		q: url.searchParams.get('q') ?? '',
		from: url.searchParams.get('from') ?? '',
		to: url.searchParams.get('to') ?? '',
		source: url.searchParams.get('source') ?? ''
	};
	try {
		const [history, inTrash, sources] = await Promise.all([
			trash ? null : getJson<HistoryPage>(fetch, `${api}/history`, filterQuery(filter)),
			getJson<HistoryPage>(fetch, `${api}/trash`),
			getJson<Source[]>(fetch, `${profileApi(me.username, profile.slug)}/sources`)
		]);
		return { profile, api, trash, filter, history, inTrash, sources };
	} catch (e) {
		if (e instanceof ApiError && e.status === 400) error(400, e.message);
		throw e;
	}
};
