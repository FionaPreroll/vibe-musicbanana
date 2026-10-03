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
import { parsePeriod, periodEnd, periodQuery } from '#lib/period.ts';
import type { PageLoad } from './$types';

// /u/<username> is the default profile, /u/<username>/<slug> any other one.
// A period in the query (see period.ts) narrows the charts to it and starts the
// listens at its end.
export const load: PageLoad = async ({ params, url, fetch }) => {
	const api = profileApi(params.username, params.slug ?? 'default');
	const period = parsePeriod(url.searchParams);
	if (!period) error(400, 'Invalid period');

	const query = { ...periodQuery(period), tz: timeZone, limit: 10 };
	const chart = (kind: ChartKind) => getJson<ChartEntry[]>(fetch, `${api}/top/${kind}`, query);

	try {
		const [overview, artists, releases, recordings, recent, nowPlaying] = await Promise.all([
			getJson<Overview>(fetch, api, { tz: timeZone }),
			chart('artists'),
			chart('releases'),
			chart('recordings'),
			getJson<ListensPage>(fetch, `${api}/listens`, {
				before: periodEnd(period)?.toISOString(),
				limit: 25
			}),
			getJson<NowPlaying | null>(fetch, `${api}/now-playing`)
		]);
		const base = profilePath(params.username, params.slug);
		return { api, base, period, overview, artists, releases, recordings, recent, nowPlaying };
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) error(404, 'There is no such profile.');
		if (e instanceof ApiError && e.status === 400) error(400, e.message);
		throw e;
	}
};
