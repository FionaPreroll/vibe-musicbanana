import { error, redirect } from '@sveltejs/kit';
import {
	ApiError,
	entityApiKinds,
	getJson,
	profileApi,
	profilePath,
	timeZone,
	type EntityPage
} from '#lib/api.ts';
import type { PageLoad } from './$types';

// /u/<username>/artist/<id>, …/album/<id> and …/track/<id>, after the slug for other
// profiles than the default one.
export const load: PageLoad = async ({ params, fetch }) => {
	const slug = params.slug ?? 'default';
	const base = profilePath(params.username, slug);
	try {
		const entity = await getJson<EntityPage>(
			fetch,
			`${profileApi(params.username, slug)}/${entityApiKinds[params.kind]}/${params.id}`,
			{ tz: timeZone }
		);
		// A merged entry answers with the one it went into; move to that one's address.
		if (entity.id !== params.id) redirect(308, `${base}/${params.kind}/${entity.id}`);
		return { base, kind: params.kind, entity };
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) {
			error(404, `There is no such profile or ${params.kind}.`);
		}
		if (e instanceof ApiError && e.status === 400) error(400, e.message);
		throw e;
	}
};
