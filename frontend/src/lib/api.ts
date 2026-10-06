import { redirect } from '@sveltejs/kit';

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

/** The week at a glance from the viewer's first weekday on, in their time zone (days as 2026-10-05). */
export type Week = {
	from: string;
	to: string;
	/** Today, or the day the week was asked for; later days have no listens yet. */
	day: string;
	days: { date: string; listens: number; last_week: number }[];
	listens: number;
	/** The week before up to the same weekday and time. */
	last_week_so_far: number;
	last_week: number;
	artists: { id: number; name: string; listens: number; last_week: number; new: boolean }[];
	/** Artists of the week never heard before it. */
	new_artists: number;
	streak: {
		current: number;
		current_from: string | null;
		longest: number;
		longest_from: string | null;
		longest_to: string | null;
	};
};

/** A year in review (backend/src/review.rs); a running year counts up to now. */
export type Review = {
	year: number;
	/** False while the year is still running. */
	complete: boolean;
	listens: number;
	/** The year before up to the same day and time; all of it for a past year. */
	last_year_so_far: number;
	last_year: number;
	artists: number;
	recordings: number;
	/** Days with listens. */
	days: number;
	/** Artists of the year never heard before it. */
	new_artists: number;
	/** The most heard of them. */
	discoveries: { id: number; name: string; listens: number; first_listened_at: string }[];
	/** Artists heard before whose listens grew the most against the year before. */
	risers: {
		id: number;
		name: string;
		listens: number;
		last_year: number;
		rank: number;
		/** `null` when not heard in the year before. */
		last_rank: number | null;
	}[];
	longest_session: {
		started_at: string;
		ended_at: string;
		listens: number;
		artists: { id: number; name: string; listens: number }[];
	} | null;
	/** January (1) to December. */
	months: { month: number; listens: number; last_year: number }[];
	top_day: { date: string; listens: number } | null;
};
/** Listens by weekday and hour in the viewer's time zone: Monday to Sunday, hours 0 to 23. */
export type Clock = { listens: number; weekdays: number[][] };

export type ChartKind = 'artists' | 'releases' | 'recordings';

/** `artist` is set for releases and recordings. */
export type ChartEntry = { id: number; name: string; artist?: string; listens: number };

/**
 * Today's date in earlier years, in the viewer's time zone: one entry per year
 * with listens on it, the latest first. 29 February looks back to the 28th.
 */
export type OnThisDay = {
	day: string;
	years: {
		date: string;
		years_ago: number;
		listens: number;
		artists: ChartEntry[];
		tracks: ChartEntry[];
	}[];
};

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
export type Me = {
	username: string;
	email: string;
	admin: boolean;
	/** The time zone dates are shown in; the browser's when null. */
	time_zone: string | null;
	/** The first day of the week, 1 for Monday to 7 for Sunday. */
	week_start: number;
	profiles: OwnProfile[];
};

/** GET /api/admin/status, see backend/src/status.rs. */
export type Status = {
	build: {
		version: string;
		commit: string | null;
		debug: boolean;
		started_at: string;
		uptime_seconds: number;
	};
	process: {
		pid: number;
		threads: number | null;
		memory_bytes: number | null;
		memory_peak_bytes: number | null;
		cpu_seconds: number | null;
		cpu_percent: number | null;
		cores: number;
		load_average: [number, number, number] | null;
		memory_total_bytes: number | null;
		memory_available_bytes: number | null;
		memory_limit_bytes: number | null;
	};
	database: {
		version: string;
		bytes: number;
		migration: number | null;
		pool_size: number;
		pool_idle: number;
		connections: number | null;
		cache_hit_percent: number | null;
		tables: { name: string; rows: number; bytes: number }[];
		last_day: { source: string; listens: number }[];
	};
	workers: { last_round_at: string | null; yourspotify: Connection[] };
	routes: {
		route: string;
		requests: number;
		server_errors: number;
		mean_ms: number;
		p50_ms: number;
		p95_ms: number;
		max_ms: number;
	}[];
};

/** The catalog's kinds as the merge API names them, see backend/src/merges.rs. */
export type CatalogKind = 'artist' | 'release' | 'recording';

/** An entry of the catalog; `artist` is set for releases and recordings, `listens` counts all profiles. */
export type CatalogEntry = { id: number; name: string; artist: string | null; listens: number };

/** GET /api/admin/merges/suggestions/<kind>: the first of `total`, and how many were hidden. */
export type MergeSuggestions = {
	total: number;
	hidden: number;
	suggestions: {
		from: CatalogEntry;
		into: CatalogEntry;
		likeness: 'same_letters' | 'one_letter' | 'version';
	}[];
};

/** A hidden suggestion; names as they were when it was hidden. */
export type HiddenSuggestion = {
	from: [number, string];
	into: [number, string];
	hidden_at: string;
};

/** What a merge changed, or would change in a dry run (`op` is null then). */
export type MergeResult = {
	kind: CatalogKind;
	from: [number, string];
	into: [number, string];
	dry_run: boolean;
	listens: number;
	spellings: number;
	releases_moved: number;
	releases_merged: number;
	recordings_moved: number;
	recordings_merged: number;
	/** Their MusicBrainz IDs tell them apart; merging needs `force`. */
	told_apart: boolean;
	op: number | null;
};

export type MergeOp = {
	id: number;
	kind: CatalogKind;
	from: [number, string];
	into: [number, string];
	merged_at: string;
	undone_at: string | null;
};

/** Pass `next` as `before` to get the following, older page. */
export type MergeLog = { merges: MergeOp[]; next: number | null };

/** Changed rows put back, and rows left as they are as they changed again since. */
export type UndoResult = { merge: MergeOp; restored: number; kept: number; dry_run: boolean };

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

/** Where the viewer stands with a profile they could follow. */
export type FollowState = 'none' | 'requested' | 'following';

export interface FollowInfo {
	username: string;
	slug: string;
	name: string;
	visibility: Visibility;
	own: boolean;
	state: FollowState;
}

/** A profile the viewer follows or asked to follow. */
export interface Followed {
	username: string;
	slug: string;
	name: string;
	visibility: Visibility;
	state: Exclude<FollowState, 'none'>;
	since: string;
}

/** Somebody who follows one of the viewer's profiles, or asks to. */
export interface Follower {
	profile: string;
	username: string;
	state: Exclude<FollowState, 'none'>;
	since: string;
}

/** A listen on the page "Edit listening history", or in the trash. */
export interface HistoryListen {
	id: number;
	listened_at: string;
	artist: string;
	track: string;
	album: string | null;
	source: string;
	trashed_at: string | null;
}

export interface HistoryPage {
	listens: HistoryListen[];
	next: string | null;
	total: number;
}

export function profileApi(username: string, slug: string) {
	return `/api/profiles/${encodeURIComponent(username)}/${encodeURIComponent(slug)}`;
}

/**
 * After a 404 on a page under /u/<username>: when the account was renamed (and
 * the viewer may see the profile), moves to the same page under the new name.
 * Returns when there is nothing to move to.
 */
export async function followRename(
	fetch: typeof globalThis.fetch,
	username: string,
	slug: string,
	url: { pathname: string; search: string }
) {
	let renamed: { username: string };
	try {
		renamed = await getJson(
			fetch,
			`/api/renamed/${encodeURIComponent(username)}/${encodeURIComponent(slug)}`
		);
	} catch {
		return;
	}
	const parts = url.pathname.split('/');
	parts[2] = encodeURIComponent(renamed.username);
	redirect(308, parts.join('/') + url.search);
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
