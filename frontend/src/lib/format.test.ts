import { describe, expect, it } from 'vitest';
import {
	byDay,
	formatCalendarDay,
	formatDate,
	formatDateRange,
	formatDateTime,
	formatDateTimeRange,
	formatDay,
	formatDuration,
	formatListenTime,
	formatMonth,
	formatNumber,
	formatPercent,
	listenCount,
	timeAgo,
	timeSince
} from '#lib/format.ts';

describe('numbers', () => {
	it('formats in the viewer locale', () => {
		expect(formatNumber(1234567)).toBe('1,234,567');
		expect(formatPercent(0.256)).toBe('26%');
		expect(listenCount(1)).toBe('1 listen');
		expect(listenCount(2048)).toBe('2,048 listens');
	});

	it('formats durations', () => {
		expect(formatDuration(45 * 60_000)).toBe('45 min');
		expect(formatDuration(3 * 3_600_000)).toBe('3 h');
		expect(formatDuration(3 * 3_600_000 + 12 * 60_000)).toBe('3 h 12 min');
		expect(formatDuration(1_500 * 3_600_000)).toBe('1,500 h');
	});
});

describe('dates', () => {
	it('shows instants in the time zone', () => {
		// 23:30 UTC is already the next day in Berlin.
		expect(formatDate('2026-10-05T23:30:00Z')).toBe('Oct 6, 2026');
		expect(formatDateTime('2026-10-05T23:30:00Z')).toBe('Oct 6, 2026, 1:30 AM');
		expect(formatDateTimeRange('2016-07-01T18:00:00Z', '2016-07-01T21:56:00Z')).toMatch(
			/^Jul 1, 2016, 8:00\s?–\s?11:56\sPM$/
		);
	});

	it('shows calendar days as they are', () => {
		expect(formatCalendarDay(new Date(2009, 5, 1))).toBe('Jun 1, 2009');
		expect(formatDateRange(new Date(2009, 5, 1), new Date(2009, 7, 31))).toMatch(
			/^Jun 1\s?–\s?Aug 31, 2009$/
		);
	});

	it('formats months, also of early years', () => {
		expect(formatMonth('2009-03')).toBe('Mar 2009');
		expect(formatMonth('0099-12')).toBe('Dec 99');
	});
});

describe('byDay', () => {
	it('groups runs of the same day in the time zone', () => {
		const items = [
			{ at: '2026-10-06T08:00:00Z' },
			{ at: '2026-10-05T22:30:00Z' }, // 00:30 on the 6th in Berlin
			{ at: '2026-10-05T21:30:00Z' },
			{ at: '2026-10-01T12:00:00Z' }
		];
		const days = byDay(items, (i) => i.at);
		expect(days.map((d) => [d.key, d.items.length])).toEqual([
			['2026-10-06', 2],
			['2026-10-05', 1],
			['2026-10-01', 1]
		]);
		expect(days[0].date).toEqual(new Date(2026, 9, 6));
	});

	it('handles no items', () => {
		expect(byDay([], () => '')).toEqual([]);
	});
});

describe('formatDay', () => {
	const now = new Date('2026-10-07T10:00:00Z'); // a Wednesday
	it('names recent days', () => {
		expect(formatDay(new Date(2026, 9, 7), now)).toBe('Today');
		expect(formatDay(new Date(2026, 9, 6), now)).toBe('Yesterday');
		expect(formatDay(new Date(2026, 9, 3), now)).toBe('Saturday');
	});

	it('gives the date further back', () => {
		expect(formatDay(new Date(2026, 8, 30), now)).toBe('Wed, September 30');
		expect(formatDay(new Date(2025, 8, 30), now)).toBe('Tue, September 30, 2025');
	});
});

describe('timeAgo', () => {
	const now = new Date('2026-10-07T10:00:00Z');
	const ago = (seconds: number) => timeAgo(new Date(now.getTime() - seconds * 1000), now);

	it('picks the largest unit', () => {
		expect(ago(10)).toBe('now');
		expect(ago(-10)).toBe('now');
		expect(ago(5 * 60)).toBe('5 minutes ago');
		expect(ago(3 * 3_600)).toBe('3 hours ago');
		expect(ago(86_400)).toBe('yesterday');
		expect(ago(8 * 86_400)).toBe('last week');
		expect(ago(65 * 86_400)).toBe('2 months ago');
		expect(ago(3 * 365 * 86_400)).toBe('3 years ago');
	});

	it('gives the time for older listens of a day', () => {
		const then = new Date(now.getTime() - 5 * 3_600_000);
		expect(formatListenTime(then, now)).toBe('5 hours ago');
		expect(formatListenTime(new Date('2026-10-07T01:15:00Z'), now)).toBe('3:15 AM');
	});
});

describe('timeSince', () => {
	const birthday = new Date('2007-08-12T14:28:00Z');

	it('gives the largest unit and the next one', () => {
		expect(timeSince(birthday, new Date('2026-10-23T12:00:00Z'))).toBe('19 years, 2 months');
		expect(timeSince(birthday, new Date('2008-08-12T14:28:00Z'))).toBe('1 year');
		expect(timeSince(birthday, new Date('2007-09-12T14:27:00Z'))).toBe('4 weeks, 2 days');
		expect(timeSince(birthday, new Date('2007-08-13T16:29:00Z'))).toBe('1 day, 2 hours');
		expect(timeSince(birthday, new Date('2007-08-12T15:29:00Z'))).toBe('1 hour, 1 minute');
		expect(timeSince(birthday, new Date('2007-08-12T14:28:30Z'))).toBe('0 minutes');
	});

	it('counts months by the calendar', () => {
		// January 31st plus one month runs over February.
		const end = new Date('2024-01-31T00:00:00Z');
		expect(timeSince(end, new Date('2024-03-01T00:00:00Z'))).toBe('4 weeks, 2 days');
		expect(timeSince(end, new Date('2024-03-31T00:00:00Z'))).toBe('2 months');
	});

	it('defaults to now', () => {
		expect(timeSince(new Date(Date.now() - 2 * 60_000))).toBe('2 minutes');
	});
});
