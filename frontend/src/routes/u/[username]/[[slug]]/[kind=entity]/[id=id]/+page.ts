import { error, redirect } from '@sveltejs/kit';
import {
	ApiError,
	entityApiKinds,
	entityPath,
	entitySegment,
	getJson,
	profileApi,
	profilePath,
	timeZone,
	type EntityPage
} from '#lib/api.ts';
import type { PageLoad } from './$types';

// /u/<username>/artist/<id>-<name>, …/album/… and …/track/…, after the slug for other
// profiles than the default one. Only the id counts; the name is for the reader.
export const load: PageLoad = async ({ params, url, fetch }) => {
	const slug = params.slug ?? 'default';
	const base = profilePath(params.username, slug);
	try {
		const entity = await getJson<EntityPage>(
			fetch,
			`${profileApi(params.username, slug)}/${entityApiKinds[params.kind]}/${params.id}`,
			{ tz: timeZone }
		);
		// Old links without the name, renamed entries, and merged ones (which answer
		// with the entry they went into) move to the current address.
		const segment = decodeURIComponent(url.pathname.split('/').at(-1) ?? '');
		if (segment !== entitySegment(entity.id, entity.name)) {
			redirect(308, entityPath(base, params.kind, entity.id, entity.name) + url.search);
		}
		return { base, kind: params.kind, entity };
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) {
			error(404, `There is no such profile or ${params.kind}.`);
		}
		if (e instanceof ApiError && e.status === 400) error(400, e.message);
		throw e;
	}
};
