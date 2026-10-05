import { error } from '@sveltejs/kit';
import {
	ApiError,
	followRename,
	getJson,
	profileApi,
	profilePath,
	type Overview,
	type SearchResult,
	type Source
} from '#lib/api.ts';
import { sourceOf } from '#lib/source.ts';
import type { PageLoad } from './$types';

// /u/<username>/search?q=… searches the artists, albums and tracks the profile has heard.
export const load: PageLoad = async ({ params, url, fetch }) => {
	const slug = params.slug ?? 'default';
	const api = profileApi(params.username, slug);
	const q = url.searchParams.get('q')?.trim() ?? '';
	const source = sourceOf(url);
	try {
		const [overview, found, sources] = await Promise.all([
			getJson<Overview>(fetch, api),
			q
				? getJson<SearchResult>(fetch, `${api}/search`, { q, limit: 20, source })
				: Promise.resolve<SearchResult>({ artists: [], releases: [], recordings: [] }),
			getJson<Source[]>(fetch, `${api}/sources`)
		]);
		return { base: profilePath(params.username, slug), overview, q, source, found, sources };
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) {
			await followRename(fetch, params.username, params.slug ?? 'default', url);
			error(404, 'There is no such profile.');
		}
		throw e;
	}
};
