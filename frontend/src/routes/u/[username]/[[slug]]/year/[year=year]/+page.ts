import { error } from '@sveltejs/kit';
import {
	ApiError,
	followRename,
	getJson,
	profileApi,
	profilePath,
	timeZone,
	type ChartEntry,
	type ChartKind,
	type Overview,
	type Review,
	type Source
} from '#lib/api.ts';
import { sourceOf } from '#lib/source.ts';
import type { PageLoad } from './$types';

// /u/<username>/year/2016 (after the slug for other profiles): the year in review,
// narrowed to a source like the other pages of a profile.
export const load: PageLoad = async ({ params, url, fetch }) => {
	const slug = params.slug ?? 'default';
	const api = profileApi(params.username, slug);
	const year = params.year;
	const source = sourceOf(url);
	const chart = (kind: ChartKind) =>
		getJson<ChartEntry[]>(fetch, `${api}/top/${kind}`, { year, tz: timeZone, limit: 10, source });
	try {
		const [overview, review, artists, releases, recordings, sources] = await Promise.all([
			getJson<Overview>(fetch, api, { tz: timeZone, source }),
			getJson<Review>(fetch, `${api}/year/${year}`, { tz: timeZone, source }),
			chart('artists'),
			chart('releases'),
			chart('recordings'),
			getJson<Source[]>(fetch, `${api}/sources`)
		]);
		return {
			base: profilePath(params.username, slug),
			source,
			sources,
			overview,
			review,
			artists,
			releases,
			recordings
		};
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) {
			await followRename(fetch, params.username, slug, url);
			error(404, 'There is no such profile.');
		}
		if (e instanceof ApiError && e.status === 400) error(400, e.message);
		throw e;
	}
};
