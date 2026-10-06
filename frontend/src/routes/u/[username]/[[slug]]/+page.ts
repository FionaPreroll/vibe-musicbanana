import { error, redirect } from '@sveltejs/kit';
import {
	ApiError,
	followRename,
	type FollowInfo,
	getJson,
	profileApi,
	profilePath,
	type ChartEntry,
	type ChartKind,
	type Clock,
	type ListensPage,
	type NowPlaying,
	type OnThisDay,
	type Overview,
	type Source,
	type Week
} from '#lib/api.ts';
import { parsePeriod, periodEnd, periodQuery } from '#lib/period.ts';
import { sourceOf } from '#lib/source.ts';
import { timeZone, weekStart } from '#lib/zone.svelte.ts';
import type { PageLoad } from './$types';

// /u/<username> is the default profile, /u/<username>/<slug> any other one.
// A period in the query (see period.ts) narrows the charts to it and starts the
// listens at its end; a source (see source.ts) narrows everything to its listens.
// Without a period the page starts with the current week at a glance and what
// was heard on today's date in earlier years.
export const load: PageLoad = async ({ params, url, fetch, parent }) => {
	const api = profileApi(params.username, params.slug ?? 'default');
	const period = parsePeriod(url.searchParams);
	if (!period) error(400, 'Invalid period');

	const source = sourceOf(url);
	// The viewer's settings (see zone.svelte.ts) come with the login.
	const { me } = await parent();
	const tz = timeZone();
	const query = { ...periodQuery(period), tz, limit: 10, source };
	const chart = (kind: ChartKind) => getJson<ChartEntry[]>(fetch, `${api}/top/${kind}`, query);

	try {
		const [
			overview,
			artists,
			releases,
			recordings,
			clock,
			recent,
			nowPlaying,
			sources,
			week,
			onThisDay
		] = await Promise.all([
			getJson<Overview>(fetch, api, { tz, source }),
			chart('artists'),
			chart('releases'),
			chart('recordings'),
			// The page works without it.
			getJson<Clock>(fetch, `${api}/clock`, query).catch(() => null),
			getJson<ListensPage>(fetch, `${api}/listens`, {
				before: periodEnd(period)?.toISOString(),
				limit: 25,
				source
			}),
			getJson<NowPlaying | null>(fetch, `${api}/now-playing`),
			getJson<Source[]>(fetch, `${api}/sources`),
			// The page works without it.
			period.kind === 'all'
				? getJson<Week>(fetch, `${api}/week`, { tz, week_start: weekStart(), source }).catch(
						() => null
					)
				: null,
			period.kind === 'all'
				? getJson<OnThisDay>(fetch, `${api}/on-this-day`, { tz, source }).catch(() => null)
				: null
		]);
		const base = profilePath(params.username, params.slug);
		// Whether the viewer follows it, for the button next to the name.
		const follow = me && !overview.own ? await getJson<FollowInfo>(fetch, `${api}/follow`) : null;
		return {
			follow,
			api,
			base,
			period,
			source,
			sources,
			overview,
			artists,
			releases,
			recordings,
			clock,
			recent,
			nowPlaying,
			week,
			onThisDay
		};
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) {
			await followRename(fetch, params.username, params.slug ?? 'default', url);
			// A profile for followers that the viewer may ask to follow.
			const follow = me
				? await getJson<FollowInfo>(fetch, `${api}/follow`).catch(() => null)
				: null;
			if (follow) redirect(307, `${profilePath(follow.username, follow.slug)}/follow`);
			error(404, 'There is no such profile.');
		}
		if (e instanceof ApiError && e.status === 400) error(400, e.message);
		throw e;
	}
};
