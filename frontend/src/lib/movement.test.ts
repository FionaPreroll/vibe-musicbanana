import { describe, expect, it } from 'vitest';
import { m } from '#lib/i18n/index.ts';
import { movement } from '#lib/movement.ts';

const entry = (rank: number | undefined, previous_rank?: number | null) => ({
	id: 1,
	name: 'Westerland',
	listens: 10,
	rank,
	previous_rank
});

describe('movement', () => {
	it('compares the place with the one before', () => {
		expect(movement(entry(2, 5))).toEqual({ kind: 'up', places: 3 });
		expect(movement(entry(5, 2))).toEqual({ kind: 'down', places: 3 });
		expect(movement(entry(4, 4))).toEqual({ kind: 'same' });
		expect(movement(entry(4, null))).toEqual({ kind: 'new' });
	});

	it('is null where the chart compares with nothing', () => {
		expect(movement(entry(1))).toBeNull();
		expect(movement(entry(undefined, 3))).toBeNull();
	});
});

describe('the movement texts', () => {
	it('counts places', () => {
		expect(m.chart.upTitle(1, 3)).toBe('Up 1 place, from 3');
		expect(m.chart.upTitle(4, 9)).toBe('Up 4 places, from 9');
		expect(m.chart.downTitle(1, 2)).toBe('Down 1 place, from 2');
		expect(m.chart.downTitle(2, 5)).toBe('Down 2 places, from 5');
	});
});
