import { error } from '@sveltejs/kit';
import {
	ApiError,
	getJson,
	profileApi,
	profilePath,
	timeZone,
	type ChartEntry,
	type ChartKind,
	type ListensPage,
	type NowPlaying,
	type Overview
} from '#lib/api.ts';
import type { PageLoad } from './$types';

// /u/<username> is the default profile, /u/<username>/<slug> any other one.
// ?year=2012 narrows the charts to that year and starts the listens at its end.
export const load: PageLoad = async ({ params, url, fetch }) => {
	const api = profileApi(params.username, params.slug ?? 'default');
	const yearParam = url.searchParams.get('year');
	const year = yearParam === null ? null : Number(yearParam);
	if (year !== null && !Number.isInteger(year)) error(400, 'Invalid year');

	const chart = (kind: ChartKind) =>
		getJson<ChartEntry[]>(fetch, `${api}/top/${kind}`, { year, tz: timeZone, limit: 10 });

	try {
		const [overview, artists, releases, recordings, recent, nowPlaying] = await Promise.all([
			getJson<Overview>(fetch, api, { tz: timeZone }),
			chart('artists'),
			chart('releases'),
			chart('recordings'),
			getJson<ListensPage>(fetch, `${api}/listens`, {
				before: year === null ? null : startOfYear(year + 1).toISOString(),
				limit: 25
			}),
			getJson<NowPlaying | null>(fetch, `${api}/now-playing`)
		]);
		const base = profilePath(params.username, params.slug);
		return { api, base, year, overview, artists, releases, recordings, recent, nowPlaying };
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) error(404, 'There is no such profile.');
		if (e instanceof ApiError && e.status === 400) error(400, e.message);
		throw e;
	}
};

/** Local midnight on January 1st; `new Date(y, 0, 1)` would map years below 100 to 19xx. */
function startOfYear(year: number) {
	const date = new Date(2000, 0, 1);
	date.setFullYear(year);
	return date;
}
