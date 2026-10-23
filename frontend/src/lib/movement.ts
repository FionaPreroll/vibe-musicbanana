// How an entry of a chart moved since the period before (see `previous_rank` in
// api.ts): up or down by some places, the same place, or new in the chart.

import type { ChartEntry } from '#lib/api.ts';

export type Movement = { kind: 'up' | 'down'; places: number } | { kind: 'same' } | { kind: 'new' };

/** The movement of an entry, or `null` when its chart compares with nothing. */
export function movement(entry: ChartEntry): Movement | null {
	if (entry.previous_rank === undefined || entry.rank === undefined) return null;
	if (entry.previous_rank === null) return { kind: 'new' };
	const places = entry.previous_rank - entry.rank;
	if (places > 0) return { kind: 'up', places };
	if (places < 0) return { kind: 'down', places: -places };
	return { kind: 'same' };
}

// The words of the chart, together so that they can move to a translation later.
export const movementText = {
	new: 'new',
	newTitle: 'Not heard in the period before',
	same: '=',
	sameTitle: 'Same place as in the period before',
	upTitle: (places: number, previous: number) =>
		`Up ${places} ${places === 1 ? 'place' : 'places'}, from ${previous}`,
	downTitle: (places: number, previous: number) =>
		`Down ${places} ${places === 1 ? 'place' : 'places'}, from ${previous}`,
	showMore: 'Show more',
	showLess: 'Show less',
	loading: 'Loading…',
	failed: 'Could not load more.',
	empty: 'Nothing here.'
};
