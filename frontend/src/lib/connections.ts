import type { Connection } from '#lib/api.ts';
import { formatDate, formatDateTime } from '#lib/format.ts';
import { m } from '#lib/i18n/index.ts';

/** Where a YourSpotify connection stands, in a sentence. */
export function connectionState(c: Connection) {
	if (!c.started_at) return m.connection.starts;
	if (!c.finished_at || c.finished_at < c.started_at) {
		return m.connection.importing(formatDateTime(c.started_at));
	}
	if (c.error) return m.connection.failed(formatDateTime(c.finished_at), c.error);
	return m.connection.upToDate(formatDateTime(c.finished_at));
}

/** Whether an import of the connection runs now. */
export const importing = (c: Connection) =>
	!!c.started_at && (!c.finished_at || c.finished_at < c.started_at);

/** How fetching the whole history again stands, in a sentence, if it was ever asked for. */
export function refetchState(c: Connection) {
	if (c.refetch_requested_at) {
		const upTo =
			c.refetch_at && new Date(c.refetch_at) < new Date() ? formatDate(c.refetch_at) : null;
		if (importing(c) && (c.started_at ?? '') >= c.refetch_requested_at) {
			return m.connection.refetching(c.refetch_plays, c.refetch_new, upTo);
		}
		if (c.refetch_plays > 0 || c.refetch_at) {
			return m.connection.refetchStopped(c.refetch_new, upTo);
		}
		return importing(c) ? m.connection.refetchAfterImport : m.connection.refetchSoon;
	}
	if (!c.refetched_at) return null;
	const there = c.refetch_plays - c.refetch_new - c.refetch_left_out;
	return m.connection.refetched(
		formatDateTime(c.refetched_at),
		c.refetch_plays,
		c.refetch_new,
		there,
		c.refetch_left_out
	);
}
