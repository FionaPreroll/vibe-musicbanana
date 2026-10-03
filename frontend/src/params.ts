import { defineParams } from '@sveltejs/kit/params';
import type { EntityKind } from '#lib/api.ts';

const entityKinds: EntityKind[] = ['artist', 'album', 'track'];

// Matchers for route parameters such as [kind=entity] and [id=id].
export const params = defineParams({
	entity: (param): EntityKind | undefined => entityKinds.find((kind) => kind === param),
	// Database ids; at most 15 digits, so they fit into a JavaScript number.
	id: (param) => (/^[1-9][0-9]{0,14}$/.test(param) ? Number(param) : undefined)
});
