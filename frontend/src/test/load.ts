import type { Me } from '#lib/api.ts';

/**
 * What SvelteKit hands a load function, for calling one in a test:
 * `await load(event({ path: '/u/fiona?year=2012', params: { username: 'fiona' }, fetch }))`.
 */
export function event({
	path = '/',
	params = {},
	me = null,
	fetch
}: {
	path?: string;
	params?: Record<string, unknown>;
	me?: Me | null;
	fetch: typeof globalThis.fetch;
}) {
	// The page types differ per route; a test builds what all of them read.
	return {
		url: new URL(path, 'http://localhost'),
		params,
		fetch,
		parent: async () => ({ me })
	} as never;
}

/** What a load function threw, e.g. a redirect or an error, to compare. */
export async function thrown(promise: unknown) {
	try {
		await promise;
	} catch (e) {
		return e;
	}
	throw new Error('Nothing was thrown');
}
