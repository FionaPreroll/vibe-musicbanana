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
	type ListensPage,
	type NowPlaying,
	type Overview,
	type Source
} from '#lib/api.ts';
import { parsePeriod, periodEnd, periodQuery } from '#lib/period.ts';
import { sourceOf } from '#lib/source.ts';
import type { PageLoad } from './$types';

// /u/<username> is the default profile, /u/<username>/<slug> any other one.
// A period in the query (see period.ts) narrows the charts to it and starts the
// listens at its end; a source (see source.ts) narrows everything to its listens.
export const load: PageLoad = async ({ params, url, fetch }) => {
	const api = profileApi(params.username, params.slug ?? 'default');
	const period = parsePeriod(url.searchParams);
	if (!period) error(400, 'Invalid period');

	const source = sourceOf(url);
	const query = { ...periodQuery(period), tz: timeZone, limit: 10, source };
	const chart = (kind: ChartKind) => getJson<ChartEntry[]>(fetch, `${api}/top/${kind}`, query);

	try {
		const [overview, artists, releases, recordings, recent, nowPlaying, sources] =
			await Promise.all([
				getJson<Overview>(fetch, api, { tz: timeZone, source }),
				chart('artists'),
				chart('releases'),
				chart('recordings'),
				getJson<ListensPage>(fetch, `${api}/listens`, {
					before: periodEnd(period)?.toISOString(),
					limit: 25,
					source
				}),
				getJson<NowPlaying | null>(fetch, `${api}/now-playing`),
				getJson<Source[]>(fetch, `${api}/sources`)
			]);
		const base = profilePath(params.username, params.slug);
		return {
			api,
			base,
			period,
			source,
			sources,
			overview,
			artists,
			releases,
			recordings,
			recent,
			nowPlaying
		};
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) {
			await followRename(fetch, params.username, params.slug ?? 'default', url);
			error(404, 'There is no such profile.');
		}
		if (e instanceof ApiError && e.status === 400) error(400, e.message);
		throw e;
	}
};
