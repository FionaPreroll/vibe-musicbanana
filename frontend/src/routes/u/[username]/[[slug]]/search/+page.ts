import { error } from '@sveltejs/kit';
import {
	ApiError,
	getJson,
	profileApi,
	profilePath,
	type Overview,
	type SearchResult
} from '#lib/api.ts';
import type { PageLoad } from './$types';

// /u/<username>/search?q=… searches the artists, albums and tracks the profile has heard.
export const load: PageLoad = async ({ params, url, fetch }) => {
	const slug = params.slug ?? 'default';
	const api = profileApi(params.username, slug);
	const q = url.searchParams.get('q')?.trim() ?? '';
	try {
		const [overview, found] = await Promise.all([
			getJson<Overview>(fetch, api),
			q
				? getJson<SearchResult>(fetch, `${api}/search`, { q, limit: 20 })
				: Promise.resolve<SearchResult>({ artists: [], releases: [], recordings: [] })
		]);
		return { base: profilePath(params.username, slug), overview, q, found };
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) error(404, 'There is no such profile.');
		throw e;
	}
};
