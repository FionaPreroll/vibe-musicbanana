// Types and small fetch helpers for the backend's JSON API (backend/src/profiles.rs,
// backend/src/entities.rs and backend/src/account.rs).

/** Who sees a profile: everybody, its owner and the accounts following it, or its owner only. */
export type Visibility = 'public' | 'followers' | 'private';

export type ProfileSummary = {
	username: string;
	slug: string;
	name: string;
	visibility: Visibility;
	listens: number;
};

export type Overview = {
	username: string;
	slug: string;
	name: string;
	visibility: Visibility;
	/** Whether it belongs to the logged-in account. */
	own: boolean;
	listens: number;
	first_listened_at: string | null;
	last_listened_at: string | null;
	years: { year: number; listens: number }[];
};

export type ChartKind = 'artists' | 'releases' | 'recordings';

/** `artist` is set for releases and recordings. */
export type ChartEntry = { id: number; name: string; artist?: string; listens: number };

/** The most heard artists of a year, best first; `listens` counts all of the year's listens. */
export type YearTop = { year: number; listens: number; artists: ChartEntry[] };

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
	/** MusicBrainz IDs: of the artist, of the album's release group, or of the track's recordings. */
	mbids: string[];
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

/** The logged-in account, with its profiles (the default one first). */
export type Me = { username: string; email: string; profiles: OwnProfile[] };

export type OwnProfile = { slug: string; name: string; visibility: Visibility; listens: number };

/** A scrobble token; the token itself is only shown once, when it is made. */
export type ApiToken = {
	id: number;
	profile: string;
	label: string;
	created_at: string;
	last_used_at: string | null;
};

/** A YourSpotify account the server imports Spotify plays from into a profile. */
export type Connection = {
	id: number;
	username: string;
	profile: string;
	url: string;
	created_at: string;
	started_at: string | null;
	finished_at: string | null;
	/** Listens it brought so far. */
	imported: number;
	/** Why the latest import failed. */
	error: string | null;
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

/** Sends JSON to the API and returns the JSON answer, or `null` for an empty one. */
export async function sendJson<T = null>(
	fetch: typeof globalThis.fetch,
	method: 'POST' | 'PUT' | 'PATCH' | 'DELETE',
	path: string,
	body?: unknown
): Promise<T> {
	const res = await fetch(path, {
		method,
		headers: body === undefined ? {} : { 'content-type': 'application/json' },
		body: body === undefined ? undefined : JSON.stringify(body)
	});
	if (!res.ok) throw new ApiError(res.status, (await res.text()) || res.statusText);
	return res.status === 204 ? (null as T) : res.json();
}

/** A message for the person in front of the screen, for any error. */
export function errorMessage(e: unknown) {
	const message = e instanceof Error ? e.message : String(e);
	return message.charAt(0).toUpperCase() + message.slice(1);
}

export function profileApi(username: string, slug: string) {
	return `/api/profiles/${encodeURIComponent(username)}/${encodeURIComponent(slug)}`;
}

/** The page of a profile: /u/<username> for the default one, /u/<username>/<slug> for others. */
export function profilePath(username: string, slug = 'default') {
	const path = `/u/${encodeURIComponent(username)}`;
	return slug === 'default' ? path : `${path}/${encodeURIComponent(slug)}`;
}

/** A name as it goes into an address: "Die Ärzte" → "die-arzte", "Jóga" → "joga". */
export function nameSlug(name: string) {
	return name
		.normalize('NFKD')
		.replace(/\p{M}/gu, '')
		.toLowerCase()
		.replace(/ß/g, 'ss')
		.replace(/[^\p{L}\p{N}]+/gu, '-')
		.slice(0, 60)
		.replace(/^-+|-+$/g, '');
}

/** The id with the name after it, as in /u/fiona/artist/1-die-arzte. Only the id counts. */
export const entitySegment = (id: number, name: string) =>
	nameSlug(name) ? `${id}-${nameSlug(name)}` : String(id);

/** The page of an artist, album or track in the profile at `base`. */
export const entityPath = (base: string, kind: EntityKind, id: number, name: string) =>
	`${base}/${kind}/${encodeURIComponent(entitySegment(id, name))}`;

/** Where listens of a profile came from; `source` is what `?source=` takes. */
export type Source = {
	source: string;
	listens: number;
	first_listened_at: string;
	last_listened_at: string;
};

/** What a profile's search found, the most heard first. */
export type SearchResult = {
	artists: ChartEntry[];
	releases: ChartEntry[];
	recordings: ChartEntry[];
};

/** The viewer's time zone, so that a year starts at their midnight. */
export const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone;
