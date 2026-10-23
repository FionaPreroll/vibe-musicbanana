// Numbers and dates in the page's locale (see i18n/locale.svelte.ts). Instants
// from the API (strings, or Dates of them) are shown in the viewer's time zone,
// calendar days (local midnight, see period.ts) as they are; see zone.svelte.ts.

import { m } from '#lib/i18n/index.ts';
import { formatNumber, localized } from '#lib/i18n/intl.ts';
import { wallClock, wallDay, zoned } from '#lib/zone.svelte.ts';

export { formatList, formatNumber, listParts } from '#lib/i18n/intl.ts';

/** A date formatter of `options` in the page's locale, for calendar days. */
export const dates = (options: Intl.DateTimeFormatOptions) =>
	localized((locale) => new Intl.DateTimeFormat(locale, options));

const date = dates({ dateStyle: 'medium' });
const zonedDate = zoned({ dateStyle: 'medium' });
const dateTime = zoned({ dateStyle: 'medium', timeStyle: 'short' });
const monthOfYear = dates({ month: 'short', year: 'numeric' });
const percent = localized((locale) => new Intl.NumberFormat(locale, { style: 'percent' }));
const time = zoned({ timeStyle: 'short' });
const weekday = dates({ weekday: 'long' });
const dayThisYear = dates({ weekday: 'short', day: 'numeric', month: 'long' });
const day = dates({ weekday: 'short', day: 'numeric', month: 'long', year: 'numeric' });
const relative = localized((locale) => new Intl.RelativeTimeFormat(locale, { numeric: 'auto' }));
const disjunction = localized((locale) => new Intl.ListFormat(locale, { type: 'disjunction' }));

/** The day of an instant from the API, in the viewer's time zone. */
export const formatDate = (iso: string) => zonedDate().format(new Date(iso));
/** A calendar day (local midnight). */
export const formatCalendarDay = (when: Date) => date().format(when);
export const formatDateRange = (from: Date, to: Date) => date().formatRange(from, to);
export const formatDateTime = (iso: string) => dateTime().format(new Date(iso));
/** "1 Jul 2016, 20:00 – 23:56", with the day once when both are on the same one. */
export const formatDateTimeRange = (from: string, to: string) =>
	dateTime().formatRange(new Date(from), new Date(to));
export const formatPercent = (share: number) => percent().format(share);
/** "A, B or C". */
export const formatOrList = (items: string[]) => disjunction().format(items);

/** "2009-03" as e.g. "Mar 2009". */
export function formatMonth(month: string) {
	const [year, mo] = month.split('-').map(Number);
	const when = new Date(2000, mo - 1, 1);
	when.setFullYear(year);
	return monthOfYear().format(when);
}

/** A stretch of time in hours and minutes, e.g. "3 h 12 min" or "45 min". */
export function formatDuration(ms: number) {
	const minutes = Math.round(ms / 60_000);
	return m.duration(Math.floor(minutes / 60), minutes % 60);
}

/** A calendar day (local midnight) like "2026-10-06". */
function dayKey(when: Date) {
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${when.getFullYear()}-${pad(when.getMonth() + 1)}-${pad(when.getDate())}`;
}

/**
 * Items in order, split into runs of the same day in the viewer's time zone,
 * each with that day as local midnight. The lists are newest first, so each
 * day comes once.
 */
export function byDay<T>(items: T[], when: (item: T) => string) {
	const days: { key: string; date: Date; items: T[] }[] = [];
	for (const item of items) {
		const date = wallDay(when(item));
		const key = dayKey(date);
		if (days.at(-1)?.key === key) days.at(-1)!.items.push(item);
		else days.push({ key, date, items: [item] });
	}
	return days;
}

const capitalized = (s: string) => s.charAt(0).toLocaleUpperCase() + s.slice(1);

/**
 * A calendar day (as from `byDay`) as a heading: "Today", "Yesterday", the
 * weekday within a week, else the date. `now` is an instant.
 */
export function formatDay(when: Date, now: Date) {
	const midnight = (d: Date) => Date.UTC(d.getFullYear(), d.getMonth(), d.getDate());
	now = wallClock(now);
	const daysAgo = Math.round((midnight(now) - midnight(when)) / 86_400_000);
	if (daysAgo === 0 || daysAgo === 1) return capitalized(relative().format(-daysAgo, 'day'));
	if (daysAgo > 1 && daysAgo < 7) return weekday().format(when);
	return (when.getFullYear() === now.getFullYear() ? dayThisYear : day)().format(when);
}

/** How long ago `then` was in one unit, e.g. "now", "5 minutes ago", "3 years ago". */
export function timeAgo(then: Date, now: Date) {
	const seconds = Math.max(0, (now.getTime() - then.getTime()) / 1000);
	const units: [Intl.RelativeTimeFormatUnit, number][] = [
		['year', 365 * 86_400],
		['month', 30 * 86_400],
		['week', 7 * 86_400],
		['day', 86_400],
		['hour', 3_600],
		['minute', 60]
	];
	for (const [unit, length] of units) {
		if (seconds >= length) return relative().format(-Math.floor(seconds / length), unit);
	}
	return relative().format(0, 'second');
}

/** When a listen happened, under its day's heading: how long ago for the last hours, else the time. */
export function formatListenTime(then: Date, now: Date) {
	return now.getTime() - then.getTime() < 6 * 3_600_000 ? timeAgo(then, now) : time().format(then);
}

/** "1 listen", "1,234 listens". */
export const listenCount = (n: number) => m.listens(n);

/**
 * How long ago `then` was, like Django's `timesince` filter: the largest unit
 * and the next one if it is not zero, e.g. "19 years, 1 month".
 */
export function timeSince(then: Date, now = new Date()) {
	// Whole calendar months first, so month lengths and leap years count right.
	let months =
		(now.getUTCFullYear() - then.getUTCFullYear()) * 12 + now.getUTCMonth() - then.getUTCMonth();
	const shifted = (k: number) => {
		const d = new Date(then);
		d.setUTCMonth(d.getUTCMonth() + k);
		return d;
	};
	// Adding months to the 31st can run over into the month after, so maybe step back twice.
	while (months > 0 && shifted(months) > now) months--;
	let rest = Math.max(0, Math.floor((now.getTime() - shifted(months).getTime()) / 60_000));
	const weeks = Math.floor(rest / (7 * 24 * 60));
	rest -= weeks * 7 * 24 * 60;
	const days = Math.floor(rest / (24 * 60));
	rest -= days * 24 * 60;
	const hours = Math.floor(rest / 60);
	const counts = [Math.floor(months / 12), months % 12, weeks, days, hours, rest - hours * 60];

	const units = m.since.units;
	const first = counts.findIndex((c) => c > 0);
	if (first < 0) return units[5](0);
	const part = (i: number) => units[i](counts[i]);
	return first + 1 < counts.length && counts[first + 1] > 0
		? m.since.two(part(first), part(first + 1))
		: part(first);
}
