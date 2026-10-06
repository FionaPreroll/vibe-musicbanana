// The filter of the page "Edit listening history".

import { fromWallClock } from '#lib/zone.svelte.ts';

export type Filter = { q: string; from: string; to: string; source: string };

/**
 * The filter as the API takes it: times as instants, empty fields left out.
 * The fields hold the day and time in the viewer's time zone.
 */
export function filterQuery(filter: Filter) {
	const instant = (field: string) => fromWallClock(new Date(field));
	return {
		q: filter.q.trim() || undefined,
		from: filter.from ? instant(filter.from).toISOString() : undefined,
		// To the end of the minute picked.
		to: filter.to ? new Date(instant(filter.to).getTime() + 59_999).toISOString() : undefined,
		source: filter.source || undefined
	};
}
