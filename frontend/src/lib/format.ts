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
