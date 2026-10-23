import { ApiError, getJson, type Me, type ServerConfig } from '#lib/api.ts';
import { useCovers } from '#lib/covers.svelte.ts';
import { useTimeSettings } from '#lib/zone.svelte.ts';
import type { LayoutLoad } from './$types';

// Pure client-side app: no Node server at runtime, the backend serves the static build.
export const ssr = false;

// Who is logged in, if anybody. Logging in or out reloads this and every page with it,
// as private profiles appear or go, and so do the time zone and first weekday.
export const load: LayoutLoad = async ({ fetch }) => {
	// What the server offers; without an answer the pages show no covers.
	const config = getJson<ServerConfig>(fetch, '/api/config').catch(() => ({ covers: false }));
	let me: Me | null;
	try {
		me = await getJson<Me>(fetch, '/api/me');
	} catch (e) {
		if (!(e instanceof ApiError && e.status === 401)) throw e;
		me = null;
	}
	useTimeSettings(me);
	useCovers((await config).covers);
	return { me };
};
