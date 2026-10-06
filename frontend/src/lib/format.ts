// Numbers and dates in the viewer's locale.

const number = new Intl.NumberFormat();
const date = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' });
const dateTime = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });
const monthOfYear = new Intl.DateTimeFormat(undefined, { month: 'short', year: 'numeric' });
const percent = new Intl.NumberFormat(undefined, { style: 'percent' });
const time = new Intl.DateTimeFormat(undefined, { timeStyle: 'short' });
const weekday = new Intl.DateTimeFormat(undefined, { weekday: 'long' });
const dayThisYear = new Intl.DateTimeFormat(undefined, {
	weekday: 'short',
	day: 'numeric',
	month: 'long'
});
const day = new Intl.DateTimeFormat(undefined, {
	weekday: 'short',
	day: 'numeric',
	month: 'long',
	year: 'numeric'
});
const relative = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });

export const formatNumber = (n: number) => number.format(n);
export const formatDate = (when: string | Date) => date.format(new Date(when));
export const formatDateRange = (from: Date, to: Date) => date.formatRange(from, to);
export const formatDateTime = (iso: string) => dateTime.format(new Date(iso));
export const formatPercent = (share: number) => percent.format(share);

/** "2009-03" as e.g. "Mar 2009". */
export function formatMonth(month: string) {
	const [year, m] = month.split('-').map(Number);
	const date = new Date(2000, m - 1, 1);
	date.setFullYear(year);
	return monthOfYear.format(date);
}

/** The viewer's calendar day of `when`, like "2026-10-06". */
export function dayKey(when: Date) {
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${when.getFullYear()}-${pad(when.getMonth() + 1)}-${pad(when.getDate())}`;
}

/**
 * Items in order, split into runs of the same calendar day. The lists are
 * newest first, so each day comes once.
 */
export function byDay<T>(items: T[], when: (item: T) => string) {
	const days: { key: string; date: Date; items: T[] }[] = [];
	for (const item of items) {
		const date = new Date(when(item));
		const key = dayKey(date);
		if (days.at(-1)?.key === key) days.at(-1)!.items.push(item);
		else days.push({ key, date, items: [item] });
	}
	return days;
}

const capitalized = (s: string) => s.charAt(0).toLocaleUpperCase() + s.slice(1);

/** A day as a heading: "Today", "Yesterday", the weekday within a week, else the date. */
export function formatDay(when: Date, now: Date) {
	const midnight = (d: Date) => Date.UTC(d.getFullYear(), d.getMonth(), d.getDate());
	const daysAgo = Math.round((midnight(now) - midnight(when)) / 86_400_000);
	if (daysAgo === 0 || daysAgo === 1) return capitalized(relative.format(-daysAgo, 'day'));
	if (daysAgo > 1 && daysAgo < 7) return weekday.format(when);
	return (when.getFullYear() === now.getFullYear() ? dayThisYear : day).format(when);
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
		if (seconds >= length) return relative.format(-Math.floor(seconds / length), unit);
	}
	return relative.format(0, 'second');
}

/** When a listen happened, under its day's heading: how long ago for the last hours, else the time. */
export function formatListenTime(then: Date, now: Date) {
	return now.getTime() - then.getTime() < 6 * 3_600_000 ? timeAgo(then, now) : time.format(then);
}

export const listenCount = (n: number) => `${number.format(n)} ${n === 1 ? 'listen' : 'listens'}`;

const sinceUnits = [
	['year', 'years'],
	['month', 'months'],
	['week', 'weeks'],
	['day', 'days'],
	['hour', 'hours'],
	['minute', 'minutes']
] as const;

/**
 * How long ago `then` was, like Django's `timesince` filter: the largest unit
 * and the next one if it is not zero, e.g. "19 years, 1 month".
 */
export function timeSince(then: Date, now = new Date()) {
	// Whole calendar months first, so month lengths and leap years count right.
	let months =
		(now.getUTCFullYear() - then.getUTCFullYear()) * 12 + now.getUTCMonth() - then.getUTCMonth();
	const shifted = (m: number) => {
		const d = new Date(then);
		d.setUTCMonth(d.getUTCMonth() + m);
		return d;
	};
	if (months > 0 && shifted(months) > now) months--;
	let rest = Math.max(0, Math.floor((now.getTime() - shifted(months).getTime()) / 60_000));
	const weeks = Math.floor(rest / (7 * 24 * 60));
	rest -= weeks * 7 * 24 * 60;
	const days = Math.floor(rest / (24 * 60));
	rest -= days * 24 * 60;
	const hours = Math.floor(rest / 60);
	const counts = [Math.floor(months / 12), months % 12, weeks, days, hours, rest - hours * 60];

	const first = counts.findIndex((n) => n > 0);
	if (first < 0) return '0 minutes';
	const part = (i: number) => `${counts[i]} ${sinceUnits[i][counts[i] === 1 ? 0 : 1]}`;
	return first + 1 < counts.length && counts[first + 1] > 0
		? `${part(first)}, ${part(first + 1)}`
		: part(first);
}
