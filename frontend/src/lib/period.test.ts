import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	isoDay,
	localDay,
	parseDay,
	parsePeriod,
	periodDays,
	periodEnd,
	periodLabel,
	periodQuery,
	periodSearch,
	samePeriod,
	type Period
} from '#lib/period.ts';

const parse = (search: string) => parsePeriod(new URLSearchParams(search));

describe('parsePeriod', () => {
	it('reads each kind', () => {
		expect(parse('')).toEqual({ kind: 'all' });
		expect(parse('year=2012')).toEqual({ kind: 'year', year: 2012 });
		expect(parse('days=30')).toEqual({ kind: 'days', days: 30 });
		expect(parse('from=2009-06-01&to=2009-08-31')).toEqual({
			kind: 'range',
			from: '2009-06-01',
			to: '2009-08-31'
		});
		expect(parse('from=2009-06-01&to=')).toEqual({ kind: 'range', from: '2009-06-01', to: null });
		expect(parse('from=&to=')).toEqual({ kind: 'all' });
	});

	it('refuses malformed values', () => {
		expect(parse('year=twelve')).toBeUndefined();
		expect(parse('days=0')).toBeUndefined();
		expect(parse('days=36501')).toBeUndefined();
		expect(parse('days=1.5')).toBeUndefined();
		expect(parse('from=1.6.2009')).toBeUndefined();
		expect(parse('to=2009-6-1')).toBeUndefined();
	});
});

describe('periodSearch', () => {
	it('round-trips through parsePeriod', () => {
		const periods: Period[] = [
			{ kind: 'all' },
			{ kind: 'year', year: 2012 },
			{ kind: 'days', days: 7 },
			{ kind: 'range', from: '2009-06-01', to: '2009-08-31' },
			{ kind: 'range', from: null, to: '2009-08-31' }
		];
		for (const period of periods) {
			expect(parse(periodSearch(period).slice(1))).toEqual(period);
		}
		expect(periodSearch({ kind: 'all' })).toBe('');
	});

	it('compares periods', () => {
		expect(samePeriod({ kind: 'days', days: 7 }, { kind: 'days', days: 7 })).toBe(true);
		expect(samePeriod({ kind: 'days', days: 7 }, { kind: 'days', days: 30 })).toBe(false);
	});
});

describe('periodDays and periodQuery', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		vi.setSystemTime(new Date('2026-10-07T10:00:00Z'));
	});
	afterEach(() => vi.useRealTimers());

	it('gives the first and last day', () => {
		expect(periodDays({ kind: 'all' })).toEqual({ from: null, to: null });
		expect(periodDays({ kind: 'year', year: 2012 })).toEqual({
			from: '2012-01-01',
			to: '2012-12-31'
		});
		expect(periodDays({ kind: 'days', days: 7 })).toEqual({ from: '2026-10-01', to: '2026-10-07' });
		expect(periodDays({ kind: 'range', from: null, to: '2009-08-31' })).toEqual({
			from: null,
			to: '2009-08-31'
		});
	});

	it('gives the parameters of the charts API', () => {
		expect(periodQuery({ kind: 'all' })).toEqual({});
		expect(periodQuery({ kind: 'year', year: 2012 })).toEqual({ year: 2012 });
		expect(periodQuery({ kind: 'days', days: 1 })).toEqual({ from: '2026-10-07' });
		expect(periodQuery({ kind: 'range', from: '2009-06-01', to: null })).toEqual({
			from: '2009-06-01',
			to: null
		});
	});
});

describe('periodEnd', () => {
	it('is midnight after the last day in the time zone', () => {
		expect(periodEnd({ kind: 'year', year: 2012 })?.toISOString()).toBe('2012-12-31T23:00:00.000Z');
		expect(periodEnd({ kind: 'range', from: null, to: '2009-08-31' })?.toISOString()).toBe(
			'2009-08-31T22:00:00.000Z'
		);
	});

	it('is null up to now', () => {
		expect(periodEnd({ kind: 'all' })).toBeNull();
		expect(periodEnd({ kind: 'days', days: 7 })).toBeNull();
		expect(periodEnd({ kind: 'range', from: '2009-06-01', to: null })).toBeNull();
	});
});

describe('periodLabel', () => {
	it('names each kind', () => {
		expect(periodLabel({ kind: 'all' })).toBe('All time');
		expect(periodLabel({ kind: 'year', year: 2012 })).toBe('2012');
		expect(periodLabel({ kind: 'days', days: 1 })).toBe('Today');
		expect(periodLabel({ kind: 'days', days: 30 })).toBe('Last 30 days');
		expect(periodLabel({ kind: 'range', from: '2009-06-01', to: '2009-08-31' })).toMatch(
			/^Jun 1\s?–\s?Aug 31, 2009$/
		);
		expect(periodLabel({ kind: 'range', from: '2009-06-01', to: null })).toBe('Since Jun 1, 2009');
		expect(periodLabel({ kind: 'range', from: null, to: '2009-08-31' })).toBe('Until Aug 31, 2009');
	});
});

describe('days', () => {
	it('keeps years below 100', () => {
		expect(localDay(42, 3, 1).getFullYear()).toBe(42);
		expect(isoDay(parseDay('0042-03-01'))).toBe('0042-03-01');
	});

	it('round-trips', () => {
		expect(isoDay(parseDay('2026-02-28'))).toBe('2026-02-28');
		expect(parseDay('2026-02-28')).toEqual(new Date(2026, 1, 28));
	});
});
