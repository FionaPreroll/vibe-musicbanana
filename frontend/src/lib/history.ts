// The filter of the page "Edit listening history".

export type Filter = { q: string; from: string; to: string; source: string };

/** The filter as the API takes it: times as instants, empty fields left out. */
export function filterQuery(filter: Filter) {
	return {
		q: filter.q.trim() || undefined,
		from: filter.from ? new Date(filter.from).toISOString() : undefined,
		// To the end of the minute picked.
		to: filter.to ? new Date(new Date(filter.to).getTime() + 59_999).toISOString() : undefined,
		source: filter.source || undefined
	};
}
