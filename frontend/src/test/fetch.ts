import { vi } from 'vitest';

type Answer = unknown | ((init: RequestInit | undefined, url: URL) => unknown);

/**
 * A fake `fetch` for the API: routes as "GET /api/me" (the path without the
 * query) to JSON answers, or to functions that get the request. A Response
 * answer goes out as it is, `undefined` as 204. Unknown routes are a 404.
 */
export function fakeApi(routes: Record<string, Answer>) {
	return vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
		const url = new URL(String(input), 'http://localhost');
		const key = `${init?.method ?? 'GET'} ${url.pathname}`;
		if (!(key in routes)) return new Response(`no route ${key}`, { status: 404 });
		const route = routes[key];
		const answer = typeof route === 'function' ? await route(init, url) : route;
		if (answer instanceof Response) return answer;
		if (answer === undefined) return new Response(null, { status: 204 });
		return Response.json(answer);
	});
}
