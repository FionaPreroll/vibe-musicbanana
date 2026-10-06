// Numbers and dates in the viewer's locale.

const number = new Intl.NumberFormat();
const date = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' });
const dateTime = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });
const monthOfYear = new Intl.DateTimeFormat(undefined, { month: 'short', year: 'numeric' });
const percent = new Intl.NumberFormat(undefined, { style: 'percent' });

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
