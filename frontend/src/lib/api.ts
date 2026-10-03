// Types and a small fetch helper for the backend's JSON API (backend/src/profiles.rs and
// backend/src/entities.rs).

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

export type Listen = {
	listened_at: string;
	artist: string;
	track: string;
	album: string | null;
	artist_id: number;
	recording_id: number;
	release_id: number | null;
};

/** Pass `next` as `before` to get the following, older page. */
export type ListensPage = { listens: Listen[]; next: string | null };

/** The page of an artist, album or track, as the URL names them. */
export type EntityKind = 'artist' | 'album' | 'track';

/** Where the API serves each kind of page. */
export const entityApiKinds: Record<EntityKind, ChartKind> = {
	artist: 'artists',
	album: 'releases',
	track: 'recordings'
};

/** "2009-03" */
export type Month = { month: string; listens: number };

export type Phase = { from: string; to: string; listens: number };

export type EntityPage = {
	profile: { username: string; slug: string; name: string };
	id: number;
	name: string;
	/** The artist of an album or track. */
	artist?: { id: number; name: string };
	listens: number;
	first_listened_at: string | null;
	last_listened_at: string | null;
	/** Every month from the profile's first listen to its last. */
	months: Month[];
	phases: Phase[];
	releases: ChartEntry[];
	recordings: ChartEntry[];
};

/** What a scrobble client reported as playing; the API sends `null` once it has run out. */
export type NowPlaying = {
	artist: string;
	track: string;
	album: string | null;
	started_at: string;
	duration_ms: number | null;
};

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

/** The page of a profile: /u/<username> for the default one, /u/<username>/<slug> for others. */
export function profilePath(username: string, slug = 'default') {
	const path = `/u/${encodeURIComponent(username)}`;
	return slug === 'default' ? path : `${path}/${encodeURIComponent(slug)}`;
}

/** The viewer's time zone, so that a year starts at their midnight. */
export const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone;
