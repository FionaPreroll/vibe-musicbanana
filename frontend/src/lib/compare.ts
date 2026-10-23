// Comparing two profiles: /u/<username>[/<slug>]/compare/<other>[/<other slug>],
// with the period of the profile page in the query (see period.ts).

import { profilePath } from '#lib/api.ts';

/** The comparison of the profile at `base` (see profilePath) with another one. */
export function comparePath(base: string, username: string, slug = 'default') {
	return `${base}/compare${profilePath(username, slug).slice('/u'.length)}`;
}

/** The API path of a comparison. */
export function compareApi(a: [string, string], b: [string, string]) {
	return '/api/compare/' + [...a, ...b].map(encodeURIComponent).join('/');
}

/** The words for an overlap from 0 to 1, the highest first. */
const levels: [number, string][] = [
	[0.5, 'Musical twins'],
	[0.3, 'Very similar taste'],
	[0.15, 'Similar taste'],
	[0.05, 'Some common ground'],
	[Number.MIN_VALUE, 'Little in common']
];

/** How an overlap reads in words. */
export function scoreLabel(score: number) {
	return levels.find(([least]) => score >= least)?.[1] ?? 'Nothing in common';
}

/**
 * Listens per year of both profiles together, for the year bar. Years without
 * listens in between stay in as gaps, unless that makes more than a century.
 */
export function combinedYears(...sides: { year: number; listens: number }[][]) {
	const years = new Map<number, number>();
	for (const side of sides) {
		for (const { year, listens } of side) years.set(year, (years.get(year) ?? 0) + listens);
	}
	const sorted = [...years.keys()].sort((a, b) => a - b);
	const first = sorted.at(0);
	const last = sorted.at(-1);
	const all =
		first === undefined || last === undefined || last - first > 100
			? sorted
			: Array.from({ length: last - first + 1 }, (_, i) => first + i);
	return all.map((year) => ({ year, listens: years.get(year) ?? 0 }));
}

/** The earlier or later of two instants from the API; null ones don't count. */
export function earliest(a: string | null, b: string | null) {
	if (a === null || b === null) return a ?? b;
	return Date.parse(a) <= Date.parse(b) ? a : b;
}

export function latest(a: string | null, b: string | null) {
	if (a === null || b === null) return a ?? b;
	return Date.parse(a) >= Date.parse(b) ? a : b;
}
