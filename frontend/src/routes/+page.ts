import { getJson, type Followed, type ProfileSummary } from '#lib/api.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, parent }) => {
	const { me } = await parent();
	const [profiles, following] = await Promise.all([
		getJson<ProfileSummary[]>(fetch, '/api/profiles'),
		me ? getJson<Followed[]>(fetch, '/api/me/following') : Promise.resolve([])
	]);
	return { profiles, following };
};
