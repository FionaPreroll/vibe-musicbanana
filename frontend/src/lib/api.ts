// Types and a small fetch helper for the backend's JSON API (backend/src/profiles.rs).

export type ProfileSummary = { username: string; slug: string; name: string; listens: number };

export type Overview = {
	username: string;
	slug: string;
	name: string;
	listens: number;
	first_listened_at: string | null;
	last_listened_at: string | null;
	years: { year: number; listens: number }[];
};

export type ChartKind = 'artists' | 'releases' | 'recordings';

/** `artist` is set for releases and recordings. */
export type ChartEntry = { id: number; name: string; artist?: string; listens: number };

export type Listen = { listened_at: string; artist: string; track: string; album: string | null };

/** Pass `next` as `before` to get the following, older page. */
export type ListensPage = { listens: Listen[]; next: string | null };

export class ApiError extends Error {
	status: number;

	constructor(status: number, message: string) {
		super(message);
		this.status = status;
	}
}

type Params = Record<string, string | number | null | undefined>;

export async function getJson<T>(
	fetch: typeof globalThis.fetch,
	path: string,
	params: Params = {}
): Promise<T> {
	const query = new URLSearchParams();
	for (const [key, value] of Object.entries(params)) {
		if (value != null) query.set(key, String(value));
	}
	const res = await fetch(query.size ? `${path}?${query}` : path);
	if (!res.ok) throw new ApiError(res.status, (await res.text()) || res.statusText);
	return res.json();
}

export function profileApi(username: string, slug: string) {
	return `/api/profiles/${encodeURIComponent(username)}/${encodeURIComponent(slug)}`;
}

/** The viewer's time zone, so that a year starts at their midnight. */
export const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone;
