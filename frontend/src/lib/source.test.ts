import { describe, expect, it } from 'vitest';
import { sourceLabel, sourceOf, withSource } from '#lib/source.ts';

describe('source', () => {
	it('reads the source from the address', () => {
		expect(sourceOf(new URL('http://x/u/fiona?source=Navidrome'))).toBe('Navidrome');
		expect(sourceOf(new URL('http://x/u/fiona'))).toBeNull();
	});
});

describe('withSource', () => {
	it('adds the source to a query string', () => {
		expect(withSource('?year=2012', 'Navidrome')).toBe('?year=2012&source=Navidrome');
		expect(withSource('', 'Spotify via YourSpotify')).toBe('?source=Spotify+via+YourSpotify');
		expect(withSource('?year=2012', null)).toBe('?year=2012');
	});
});

describe('sourceLabel', () => {
	it('names sources for people', () => {
		expect(sourceLabel('')).toBe('Unknown player');
		expect(sourceLabel('Spotify via YourSpotify')).toBe('Spotify');
		expect(sourceLabel('import:php:2016')).toBe('Old musicbanana');
		expect(sourceLabel('Navidrome')).toBe('Navidrome');
	});
});
