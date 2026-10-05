import { error, redirect } from '@sveltejs/kit';
import {
	ApiError,
	followRename,
	entityApiKinds,
	entityPath,
	entitySegment,
	getJson,
	profileApi,
	profilePath,
	timeZone,
	type EntityPage,
	type Source
} from '#lib/api.ts';
import { sourceOf } from '#lib/source.ts';
import type { PageLoad } from './$types';

// /u/<username>/artist/<id>-<name>, …/album/… and …/track/…, after the slug for other
// profiles than the default one. Only the id counts; the name is for the reader.
export const load: PageLoad = async ({ params, url, fetch }) => {
	const slug = params.slug ?? 'default';
	const base = profilePath(params.username, slug);
	try {
		const api = profileApi(params.username, slug);
		const source = sourceOf(url);
		const [entity, sources] = await Promise.all([
			getJson<EntityPage>(fetch, `${api}/${entityApiKinds[params.kind]}/${params.id}`, {
				tz: timeZone,
				source
			}),
			getJson<Source[]>(fetch, `${api}/sources`)
		]);
		// Old links without the name, renamed entries, and merged ones (which answer
		// with the entry they went into) move to the current address.
		const segment = decodeURIComponent(url.pathname.split('/').at(-1) ?? '');
		if (segment !== entitySegment(entity.id, entity.name)) {
			redirect(308, entityPath(base, params.kind, entity.id, entity.name) + url.search);
		}
		return { base, kind: params.kind, entity, source, sources };
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) {
			await followRename(fetch, params.username, slug, url);
			error(404, `There is no such profile or ${params.kind}.`);
		}
		if (e instanceof ApiError && e.status === 400) error(400, e.message);
		throw e;
	}
};
