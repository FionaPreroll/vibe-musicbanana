import { error } from '@sveltejs/kit';
import {
	ApiError,
	getJson,
	profileApi,
	profilePath,
	type Comparison,
	type Overview
} from '#lib/api.ts';
import { compareApi } from '#lib/compare.ts';
import { parsePeriod, periodQuery } from '#lib/period.ts';
import { sourceOf } from '#lib/source.ts';
import { timeZone } from '#lib/zone.svelte.ts';
import { m } from '#lib/i18n/index.ts';
import type { PageLoad } from './$types';

// /u/<username>[/<slug>]/compare/<other>[/<other slug>]: two profiles side by
// side, for the period in the query (see period.ts). The viewer must be allowed
// to see both; the API answers 404 otherwise, as for an unknown profile.
export const load: PageLoad = async ({ params, url, fetch }) => {
	const a: [string, string] = [params.username, params.slug ?? 'default'];
	const b: [string, string] = [params.other, params.otherSlug ?? 'default'];
	const period = parsePeriod(url.searchParams);
	if (!period) error(400, m.errors.invalidPeriod);
	const source = sourceOf(url);
	const tz = timeZone();

	try {
		const [comparison, overviewA, overviewB] = await Promise.all([
			getJson<Comparison>(fetch, compareApi(a, b), {
				...periodQuery(period),
				tz,
				limit: 10,
				source
			}),
			// For the year bar and the custom period's first and last day.
			getJson<Overview>(fetch, profileApi(...a), { tz, source }),
			getJson<Overview>(fetch, profileApi(...b), { tz, source })
		]);
		return {
			period,
			source,
			comparison,
			overviews: [overviewA, overviewB] as const,
			bases: [profilePath(...a), profilePath(...b)] as const
		};
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) error(404, m.errors.noProfile);
		if (e instanceof ApiError && e.status === 400) error(400, e.message);
		throw e;
	}
};
