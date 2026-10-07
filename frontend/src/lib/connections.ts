import type { Connection } from '#lib/api.ts';
import { formatDate, formatDateTime, formatNumber } from '#lib/format.ts';

/** Where a YourSpotify connection stands, in a sentence. */
export function connectionState(c: Connection) {
	if (!c.started_at) return 'Starts within a minute.';
	if (!c.finished_at || c.finished_at < c.started_at) {
		return `Importing since ${formatDateTime(c.started_at)}…`;
	}
	if (c.error) return `Failed at ${formatDateTime(c.finished_at)}: ${c.error}`;
	return `Up to date as of ${formatDateTime(c.finished_at)}.`;
}

/** Whether an import of the connection runs now. */
export const importing = (c: Connection) =>
	!!c.started_at && (!c.finished_at || c.finished_at < c.started_at);

/** How fetching the whole history again stands, in a sentence, if it was ever asked for. */
export function refetchState(c: Connection) {
	const plays = formatNumber(c.refetch_plays);
	const added = formatNumber(c.refetch_new);
	if (c.refetch_requested_at) {
		const upTo =
			c.refetch_at && new Date(c.refetch_at) < new Date()
				? `, up to ${formatDate(c.refetch_at)}`
				: '';
		if (importing(c) && (c.started_at ?? '') >= c.refetch_requested_at) {
			return `Fetching everything again: ${plays} plays so far, ${added} of them new${upTo}…`;
		}
		if (c.refetch_plays > 0 || c.refetch_at) {
			return `Fetching everything again stopped${upTo} with ${added} new so far; it goes on in the next round.`;
		}
		return importing(c)
			? 'Fetches everything again once this import is done.'
			: 'Fetches everything again within a minute.';
	}
	if (!c.refetched_at) return null;
	const there = c.refetch_plays - c.refetch_new - c.refetch_left_out;
	let text = `Fetched everything again on ${formatDateTime(c.refetched_at)}: ${plays} plays, ${added} new, ${formatNumber(there)} already here`;
	if (c.refetch_left_out > 0) {
		text += `, ${formatNumber(c.refetch_left_out)} left out as YourSpotify knows none of their artists`;
	}
	return `${text}.`;
}
