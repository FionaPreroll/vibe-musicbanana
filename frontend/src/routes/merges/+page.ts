import { error, redirect } from '@sveltejs/kit';
import {
	ApiError,
	getJson,
	type CatalogKind,
	type MergeLog,
	type MergeSuggestions
} from '#lib/api.ts';
import type { PageLoad } from './$types';

const kinds: CatalogKind[] = ['artist', 'release', 'recording'];

// The suggestions of one kind (?kind=, artists by default) and the latest merges of all kinds.
export const load: PageLoad = async ({ parent, url, fetch }) => {
	const { me } = await parent();
	if (!me) redirect(307, `/login?next=${encodeURIComponent(url.pathname + url.search)}`);
	const asked = url.searchParams.get('kind');
	const kind = kinds.find((k) => k === asked) ?? 'artist';
	try {
		const [suggestions, log] = await Promise.all([
			getJson<MergeSuggestions>(fetch, `/api/admin/merges/suggestions/${kind}`),
			getJson<MergeLog>(fetch, '/api/admin/merges')
		]);
		return { kind, suggestions, log };
	} catch (e) {
		if (e instanceof ApiError && e.status === 403) error(403, 'Merges are for admins.');
		throw e;
	}
};
