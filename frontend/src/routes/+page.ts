import { getJson, type ProfileSummary } from '#lib/api.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => ({
	profiles: await getJson<ProfileSummary[]>(fetch, '/api/profiles')
});
