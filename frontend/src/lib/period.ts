// The period the profile page shows charts for. The URL names it as ?year=2012,
// ?days=30 (the last 30 days, today included) or ?from=2009-06-01&to=2009-08-31,
// where either end may be left out; with none of them it is all time.

import { formatDate, formatDateRange } from '#lib/format.ts';

export type Period =
	| { kind: 'all' }
	| { kind: 'year'; year: number }
	| { kind: 'days'; days: number }
	| { kind: 'range'; from: string | null; to: string | null };

/** The choices offered next to "All time". */
export const recentDays = [7, 30, 90, 365];

const dayPattern = /^\d{4}-\d{2}-\d{2}$/;

/** The period in the URL, or undefined when a value is malformed. */
export function parsePeriod(search: URLSearchParams): Period | undefined {
	const year = search.get('year');
	if (year !== null) {
		const n = Number(year);
		return Number.isInteger(n) ? { kind: 'year', year: n } : undefined;
	}
	const days = search.get('days');
	if (days !== null) {
		const n = Number(days);
		return Number.isInteger(n) && n >= 1 && n <= 36500 ? { kind: 'days', days: n } : undefined;
	}
	// An empty date field of the custom form leaves that end open.
	const from = search.get('from') || null;
	const to = search.get('to') || null;
	if (from === null && to === null) return { kind: 'all' };
	if ([from, to].some((day) => day !== null && !dayPattern.test(day))) return undefined;
	return { kind: 'range', from, to };
}

/** The query string of the profile page for a period, empty for all time. */
export function periodSearch(period: Period) {
	switch (period.kind) {
		case 'all':
			return '';
		case 'year':
			return `?year=${period.year}`;
		case 'days':
			return `?days=${period.days}`;
		case 'range': {
			const search = new URLSearchParams();
			if (period.from) search.set('from', period.from);
			if (period.to) search.set('to', period.to);
			return `?${search}`;
		}
	}
}

export const samePeriod = (a: Period, b: Period) => periodSearch(a) === periodSearch(b);

/** The first and last day of the period, `null` where it is open. */
export function periodDays(period: Period, today = new Date()) {
	switch (period.kind) {
		case 'all':
			return { from: null, to: null };
		case 'year':
			return {
				from: isoDay(localDay(period.year, 1, 1)),
				to: isoDay(localDay(period.year, 12, 31))
			};
		case 'days':
			return { from: isoDay(addDays(today, 1 - period.days)), to: isoDay(today) };
		case 'range':
			return { from: period.from, to: period.to };
	}
}

/** The parameters of the charts API for a period. */
export function periodQuery(period: Period) {
	switch (period.kind) {
		case 'all':
			return {};
		case 'year':
			return { year: period.year };
		case 'days':
			// Open at the end, so that listens of the next minutes count too.
			return { from: periodDays(period).from };
		case 'range':
			return { from: period.from, to: period.to };
	}
}

/** Midnight after the last day of the period, or null when it runs up to now. */
export function periodEnd(period: Period): Date | null {
	if (period.kind === 'year') return localDay(period.year + 1, 1, 1);
	if (period.kind === 'range' && period.to !== null) return addDays(parseDay(period.to), 1);
	return null;
}

export function periodLabel(period: Period) {
	switch (period.kind) {
		case 'all':
			return 'All time';
		case 'year':
			return String(period.year);
		case 'days':
			return period.days === 1 ? 'Today' : `Last ${period.days} days`;
		case 'range':
			if (period.from && period.to) {
				return formatDateRange(parseDay(period.from), parseDay(period.to));
			}
			return period.from
				? `Since ${formatDate(parseDay(period.from))}`
				: `Until ${formatDate(parseDay(period.to ?? ''))}`;
	}
}

/** Local midnight at the start of a day; `new Date(y, m, d)` would map years below 100 to 19xx. */
export function localDay(year: number, month: number, day: number) {
	const date = new Date(2000, 0, 1);
	date.setFullYear(year, month - 1, day);
	return date;
}

/** "2009-06-01" as local midnight at the start of that day. */
export function parseDay(day: string) {
	const [year, month, date] = day.split('-').map(Number);
	return localDay(year, month, date);
}

/** The local day of a date as "2009-06-01". */
export function isoDay(date: Date) {
	const pad = (n: number, width: number) => String(n).padStart(width, '0');
	return `${pad(date.getFullYear(), 4)}-${pad(date.getMonth() + 1, 2)}-${pad(date.getDate(), 2)}`;
}

function addDays(date: Date, days: number) {
	const result = new Date(date);
	result.setDate(result.getDate() + days);
	return result;
}
