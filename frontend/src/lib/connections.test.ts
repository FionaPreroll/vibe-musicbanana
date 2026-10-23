import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Connection } from '#lib/api.ts';
import { connectionState, importing, refetchState } from '#lib/connections.ts';

const connection = (fields: Partial<Connection> = {}): Connection => ({
	id: 1,
	username: 'fiona',
	profile: 'default',
	url: 'https://yourspotify.example',
	created_at: '2026-10-01T10:00:00Z',
	started_at: '2026-10-07T09:00:00Z',
	finished_at: '2026-10-07T09:01:00Z',
	imported: 100,
	error: null,
	refetch_requested_at: null,
	refetch_at: null,
	refetch_plays: 0,
	refetch_new: 0,
	refetch_left_out: 0,
	refetched_at: null,
	...fields
});

beforeEach(() => {
	vi.useFakeTimers();
	vi.setSystemTime(new Date('2026-10-07T10:00:00Z'));
});
afterEach(() => vi.useRealTimers());

describe('connectionState', () => {
	it('tells where an import stands', () => {
		expect(connectionState(connection({ started_at: null, finished_at: null }))).toBe(
			'Starts within a minute.'
		);
		expect(connectionState(connection({ finished_at: null }))).toBe(
			'Importing since Oct 7, 2026, 11:00 AM…'
		);
		expect(connectionState(connection({ finished_at: '2026-10-07T08:00:00Z' }))).toBe(
			'Importing since Oct 7, 2026, 11:00 AM…'
		);
		expect(connectionState(connection({ error: 'connection refused' }))).toBe(
			'Failed at Oct 7, 2026, 11:01 AM: connection refused'
		);
		expect(connectionState(connection())).toBe('Up to date as of Oct 7, 2026, 11:01 AM.');
	});

	it('knows when an import runs', () => {
		expect(importing(connection({ started_at: null, finished_at: null }))).toBe(false);
		expect(importing(connection({ finished_at: null }))).toBe(true);
		expect(importing(connection())).toBe(false);
	});
});

describe('refetchState', () => {
	it('is null when never asked for', () => {
		expect(refetchState(connection())).toBeNull();
	});

	it('waits for its turn', () => {
		const asked = { refetch_requested_at: '2026-10-07T09:30:00Z' };
		expect(refetchState(connection(asked))).toBe('Fetches everything again within a minute.');
		expect(refetchState(connection({ ...asked, finished_at: null }))).toBe(
			'Fetches everything again once this import is done.'
		);
	});

	it('counts while it runs', () => {
		expect(
			refetchState(
				connection({
					refetch_requested_at: '2026-10-07T08:30:00Z',
					finished_at: null,
					refetch_plays: 1200,
					refetch_new: 30,
					refetch_at: '2020-05-01T00:00:00Z'
				})
			)
		).toBe('Fetching everything again: 1,200 plays so far, 30 of them new, up to May 1, 2020…');
	});

	it('tells when it stopped in between', () => {
		expect(
			refetchState(
				connection({
					refetch_requested_at: '2026-10-07T08:30:00Z',
					refetch_plays: 1200,
					refetch_new: 30,
					refetch_at: '2030-01-01T00:00:00Z'
				})
			)
		).toBe('Fetching everything again stopped with 30 new so far; it goes on in the next round.');
	});

	it('sums up the last time', () => {
		const done = {
			refetched_at: '2026-10-06T12:00:00Z',
			refetch_plays: 5000,
			refetch_new: 1000
		};
		expect(refetchState(connection(done))).toBe(
			'Fetched everything again on Oct 6, 2026, 2:00 PM: 5,000 plays, 1,000 new, 4,000 already here.'
		);
		expect(refetchState(connection({ ...done, refetch_left_out: 20 }))).toBe(
			'Fetched everything again on Oct 6, 2026, 2:00 PM: 5,000 plays, 1,000 new, 3,980 already here, 20 left out as YourSpotify knows none of their artists.'
		);
	});
});
