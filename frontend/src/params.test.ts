import { describe, expect, it } from 'vitest';
import { params } from './params.ts';

/** What a matcher makes of a route parameter, undefined when it does not match. */
function match(name: keyof typeof params, param: string) {
	const result = params[name]['~standard'].validate(param) as { value?: unknown };
	return result.value;
}

describe('route parameters', () => {
	it('matches entity kinds', () => {
		expect(match('entity', 'artist')).toBe('artist');
		expect(match('entity', 'track')).toBe('track');
		expect(match('entity', 'recording')).toBeUndefined();
	});

	it('matches ids with an optional name', () => {
		expect(match('id', '12')).toBe(12);
		expect(match('id', '12-die-arzte')).toBe(12);
		expect(match('id', '12-line\nbreak')).toBe(12);
		expect(match('id', '012')).toBeUndefined();
		expect(match('id', 'die-arzte')).toBeUndefined();
		expect(match('id', '1234567890123456')).toBeUndefined();
	});

	it('matches years 1 to 9999', () => {
		expect(match('year', '2012')).toBe(2012);
		expect(match('year', '7')).toBe(7);
		expect(match('year', '0')).toBeUndefined();
		expect(match('year', '10000')).toBeUndefined();
	});
});
