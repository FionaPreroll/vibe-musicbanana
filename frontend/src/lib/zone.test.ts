import { afterEach, describe, expect, it } from 'vitest';
import type { Me } from '#lib/api.ts';
import {
	browserTimeZone,
	fromWallClock,
	timeZone,
	useTimeSettings,
	wallClock,
	wallDay,
	weekStart,
	zoned,
	zoneNames
} from '#lib/zone.svelte.ts';

const me = (time_zone: string | null, week_start: number | null) =>
	({ time_zone, week_start }) as unknown as Me;

afterEach(() => useTimeSettings(null));

describe('settings', () => {
	it('defaults to the browser and Monday', () => {
		expect(browserTimeZone).toBe('Europe/Berlin');
		expect(timeZone()).toBe('Europe/Berlin');
		expect(weekStart()).toBe(1);
	});

	it('takes the account settings', () => {
		useTimeSettings(me('America/New_York', 7));
		expect(timeZone()).toBe('America/New_York');
		expect(weekStart()).toBe(7);
	});

	it('falls back for a zone the browser does not know', () => {
		useTimeSettings(me('Mars/Olympus_Mons', null));
		expect(timeZone()).toBe('Europe/Berlin');
		expect(weekStart()).toBe(1);
	});

	it('offers zones by their current names', () => {
		const names = zoneNames();
		expect(names).toContain('Asia/Kolkata');
		expect(names).not.toContain('Asia/Calcutta');
		expect([...names].sort()).toEqual(names);
	});
});

describe('wall clock', () => {
	it('shows an instant as day and time in the zone', () => {
		useTimeSettings(me('Asia/Tokyo', 1));
		expect(wallClock('2026-10-05T20:30:00Z')).toEqual(new Date(2026, 9, 6, 5, 30));
		expect(wallDay('2026-10-05T20:30:00Z')).toEqual(new Date(2026, 9, 6));
	});

	it('handles years before Christ and below 100', () => {
		useTimeSettings(me('UTC', 1));
		const instant = new Date(0);
		instant.setUTCFullYear(-5, 0, 1);
		expect(wallClock(instant).getFullYear()).toBe(-5);
		const early = new Date(0);
		early.setUTCFullYear(42, 5, 1);
		expect(wallDay(early).getFullYear()).toBe(42);
	});

	it('turns a wall clock time back into the instant', () => {
		useTimeSettings(me('America/New_York', 1));
		expect(fromWallClock(new Date(2026, 6, 1, 12, 0)).toISOString()).toBe(
			'2026-07-01T16:00:00.000Z'
		);
		expect(fromWallClock(new Date(2026, 0, 1, 12, 0)).toISOString()).toBe(
			'2026-01-01T17:00:00.000Z'
		);
		// The day summer time starts: 03:00 exists, 02:30 does not.
		expect(fromWallClock(new Date(2026, 2, 8, 3, 0)).toISOString()).toBe(
			'2026-03-08T07:00:00.000Z'
		);
	});

	it('round-trips', () => {
		useTimeSettings(me('Australia/Lord_Howe', 1));
		const instant = new Date('2026-04-04T12:34:56Z');
		expect(fromWallClock(wallClock(instant))).toEqual(instant);
	});
});

describe('zoned', () => {
	it('follows the time zone', () => {
		const format = zoned({ hour: 'numeric', hourCycle: 'h23' }, 'en-US');
		const instant = new Date('2026-10-05T12:00:00Z');
		expect(format().format(instant)).toBe('14');
		const first = format();
		expect(format()).toBe(first);
		useTimeSettings(me('UTC', 1));
		expect(format().format(instant)).toBe('12');
	});
});
