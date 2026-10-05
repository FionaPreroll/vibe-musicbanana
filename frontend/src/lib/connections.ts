import type { Connection } from '#lib/api.ts';
import { formatDateTime } from '#lib/format.ts';

/** Where a YourSpotify connection stands, in a sentence. */
export function connectionState(c: Connection) {
	if (!c.started_at) return 'Starts within a minute.';
	if (!c.finished_at || c.finished_at < c.started_at) {
		return `Importing since ${formatDateTime(c.started_at)}…`;
	}
	if (c.error) return `Failed at ${formatDateTime(c.finished_at)}: ${c.error}`;
	return `Up to date as of ${formatDateTime(c.finished_at)}.`;
}
