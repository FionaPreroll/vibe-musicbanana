// Album covers (backend/src/covers.rs). The server fetches them from the Cover
// Art Archive only when its admin switched that on; GET /api/config says so.
// While off, pages show no covers and no placeholders, as before.

let enabled = $state(false);

/** Set by the layout from GET /api/config. */
export function useCovers(on: boolean) {
	enabled = on;
}

/** Whether pages show covers (and placeholders where there is none). */
export const coversShown = () => enabled;

/** The address of an album's cover, under a profile's API (see `profileApi`). */
export function coverUrl(api: string, releaseId: number, size: 250 | 500 = 250) {
	return `${api}/releases/${releaseId}/cover${size === 250 ? '' : `?size=${size}`}`;
}

/** The first letter of a name for the placeholder, e.g. "Ä" for "Ärzte". */
export function initial(name: string) {
	const letter = Array.from(name.trim().replace(/^[^\p{L}\p{N}]+/u, ''))[0];
	return (letter ?? '?').toLocaleUpperCase();
}
