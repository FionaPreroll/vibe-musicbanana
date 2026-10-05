import { ApiError, getJson, type Me } from '#lib/api.ts';
import type { LayoutLoad } from './$types';

// Pure client-side app: no Node server at runtime, the backend serves the static build.
export const ssr = false;

// Who is logged in, if anybody. Logging in or out reloads this and every page with it,
// as private profiles appear or go.
export const load: LayoutLoad = async ({ fetch }) => {
	try {
		return { me: await getJson<Me>(fetch, '/api/me') };
	} catch (e) {
		if (e instanceof ApiError && e.status === 401) return { me: null };
		throw e;
	}
};
