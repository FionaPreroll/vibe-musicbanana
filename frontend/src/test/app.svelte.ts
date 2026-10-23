// Stand-ins for SvelteKit's $app modules, for components outside a running app:
//   vi.mock('$app/state', () => import('../test/app.svelte.ts'));
//   vi.mock('$app/navigation', () => import('../test/app.svelte.ts'));
// Tests set `page.url` and friends, and check what `goto` and the rest were called with.

import { vi } from 'vitest';

export const page = $state({
	url: new URL('http://localhost/'),
	params: {} as Record<string, string>,
	route: { id: null as string | null },
	data: {} as Record<string, unknown>,
	status: 200,
	error: null as { message: string } | null
});

/** Points `page` at another address, as a navigation would. */
export function visit(path: string) {
	page.url = new URL(path, 'http://localhost');
}

export const goto = vi.fn(async (href: string) => visit(href));
export const invalidateAll = vi.fn(async () => {});
export const afterNavigate = vi.fn();
