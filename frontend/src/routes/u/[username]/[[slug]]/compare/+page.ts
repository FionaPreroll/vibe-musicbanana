import { error } from '@sveltejs/kit';
import {
	ApiError,
	followRename,
	getJson,
	profileApi,
	profilePath,
	type Overview,
	type ProfileSummary
} from '#lib/api.ts';
import { m } from '#lib/i18n/index.ts';
import type { PageLoad } from './$types';

// /u/<username>/compare: which profile to compare this one with. The viewer's
// own profiles come first, then all others they may see.
export const load: PageLoad = async ({ params, url, fetch, parent }) => {
	const slug = params.slug ?? 'default';
	const { me } = await parent();
	try {
		const [overview, profiles] = await Promise.all([
			getJson<Overview>(fetch, profileApi(params.username, slug)),
			getJson<ProfileSummary[]>(fetch, '/api/profiles')
		]);
		const others = profiles.filter(
			(p) =>
				p.listens > 0 &&
				!(p.username === overview.username && p.slug.toLowerCase() === overview.slug.toLowerCase())
		);
		const mine = (p: ProfileSummary) => me !== null && p.username === me.username;
		return {
			base: profilePath(params.username, slug),
			overview,
			own: others.filter(mine),
			others: others.filter((p) => !mine(p))
		};
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) {
			await followRename(fetch, params.username, slug, url);
			error(404, m.errors.noProfile);
		}
		throw e;
	}
};
