import { describe, expect, it } from 'vitest';
import { filterQuery } from '#lib/history.ts';

describe('filterQuery', () => {
	it('leaves empty fields out', () => {
		expect(filterQuery({ q: '  ', from: '', to: '', source: '' })).toEqual({
			q: undefined,
			from: undefined,
			to: undefined,
			source: undefined
		});
	});

	it('turns wall clock times into instants up to the end of the minute', () => {
		expect(
			filterQuery({
				q: ' Die Ärzte ',
				from: '2026-10-05T20:00',
				to: '2026-10-05T21:30',
				source: 'Navidrome'
			})
		).toEqual({
			q: 'Die Ärzte',
			from: '2026-10-05T18:00:00.000Z',
			to: '2026-10-05T19:30:59.999Z',
			source: 'Navidrome'
		});
	});
});
