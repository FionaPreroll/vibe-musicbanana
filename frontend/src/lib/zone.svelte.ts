// The time zone and first day of the week dates are shown in: what the logged-in
// account picked in the settings, on its own profiles and on everybody else's;
// else the browser's time zone and Monday.
//
// Two kinds of Date go around in the frontend. Listen times and other instants
// come from the API and are formatted in `timeZone()`. Calendar days (see
// period.ts) are local midnight in the browser, only a holder for year, month
// and day, and are formatted without a time zone. `wallClock` turns an instant
// into such a holder for the day and time it was in `timeZone()`.

import type { Me } from '#lib/api.ts';

// Browsers name some zones by their old names (Chrome says Asia/Calcutta in
// India), which PostgreSQL only knows with the tz database's "backward" links,
// and Debian ships those apart (tzdata-legacy). These are the old names
// Intl.supportedValuesOf('timeZone') gives with their current ones.
const currentNames: Record<string, string> = {
	'Africa/Asmera': 'Africa/Asmara',
	'America/Buenos_Aires': 'America/Argentina/Buenos_Aires',
	'America/Catamarca': 'America/Argentina/Catamarca',
	'America/Cordoba': 'America/Argentina/Cordoba',
	'America/Godthab': 'America/Nuuk',
	'America/Indianapolis': 'America/Indiana/Indianapolis',
	'America/Jujuy': 'America/Argentina/Jujuy',
	'America/Louisville': 'America/Kentucky/Louisville',
	'America/Mendoza': 'America/Argentina/Mendoza',
	'Asia/Calcutta': 'Asia/Kolkata',
	'Asia/Katmandu': 'Asia/Kathmandu',
	'Asia/Rangoon': 'Asia/Yangon',
	'Asia/Saigon': 'Asia/Ho_Chi_Minh',
	'Atlantic/Faeroe': 'Atlantic/Faroe',
	'Europe/Kiev': 'Europe/Kyiv',
	'Pacific/Enderbury': 'Pacific/Kanton',
	'Pacific/Ponape': 'Pacific/Pohnpei',
	'Pacific/Truk': 'Pacific/Chuuk'
};

/** The current name of a time zone, e.g. Asia/Kolkata for Asia/Calcutta. */
const currentZoneName = (zone: string) => currentNames[zone] ?? zone;

/** The browser's time zone, e.g. Europe/Berlin. */
export const browserTimeZone = currentZoneName(Intl.DateTimeFormat().resolvedOptions().timeZone);

/** The time zones to pick from, by their current names. */
export const zoneNames = () =>
	[...new Set(Intl.supportedValuesOf('timeZone').map(currentZoneName))].sort();

const current = $state({ zone: browserTimeZone, weekStart: 1 });

/** The time zone dates are shown in. Reactive in components. */
export const timeZone = () => current.zone;

/** The first day of the week, 1 for Monday to 7 for Sunday. Reactive in components. */
export const weekStart = () => current.weekStart;

/** Takes the settings of the logged-in account, or the defaults for nobody. */
export function useTimeSettings(me: Me | null) {
	current.zone = me?.time_zone && knownZone(me.time_zone) ? me.time_zone : browserTimeZone;
	current.weekStart = me?.week_start ?? 1;
}

/** Whether the browser knows the time zone; PostgreSQL's list might have one more. */
function knownZone(zone: string) {
	try {
		new Intl.DateTimeFormat(undefined, { timeZone: zone });
		return true;
	} catch {
		return false;
	}
}

/** A formatter of `options` in the current time zone, made once per zone. */
export function zoned(options: Intl.DateTimeFormatOptions, locale?: string) {
	let made: { zone: string; format: Intl.DateTimeFormat } | undefined;
	return () => {
		const zone = current.zone;
		if (made?.zone !== zone) {
			made = { zone, format: new Intl.DateTimeFormat(locale, { ...options, timeZone: zone }) };
		}
		return made.format;
	};
}

const fields = zoned(
	{
		year: 'numeric',
		month: 'numeric',
		day: 'numeric',
		hour: 'numeric',
		minute: 'numeric',
		second: 'numeric',
		hourCycle: 'h23',
		era: 'short'
	},
	'en-US'
);

/** Year, month, day, hour, minute and second of `instant` in the current time zone. */
function wallFields(instant: Date) {
	const parts = Object.fromEntries(
		fields()
			.formatToParts(instant)
			.map((p) => [p.type, p.value])
	);
	const year = Number(parts.year);
	return [
		parts.era === 'BC' ? 1 - year : year,
		Number(parts.month),
		Number(parts.day),
		Number(parts.hour),
		Number(parts.minute),
		Number(parts.second)
	] as const;
}

/** A local Date with the day and time `instant` had in the current time zone. */
export function wallClock(instant: Date | string) {
	const [year, month, day, hour, minute, second] = wallFields(new Date(instant));
	const date = new Date(2000, 0, 1);
	date.setFullYear(year, month - 1, day);
	date.setHours(hour, minute, second, 0);
	return date;
}

/** Local midnight of `instant`'s day in the current time zone. */
export function wallDay(instant: Date | string) {
	const date = wallClock(instant);
	date.setHours(0, 0, 0, 0);
	return date;
}

/** Today in the current time zone, as local midnight. */
export const today = () => wallDay(new Date());

/** The instant at which the current time zone shows the day and time of local `wall`. */
export function fromWallClock(wall: Date) {
	const asUtc = (y: number, mo: number, d: number, h: number, mi: number, s: number) => {
		const date = new Date(0);
		date.setUTCFullYear(y, mo - 1, d);
		date.setUTCHours(h, mi, s, 0);
		return date.getTime();
	};
	const target = asUtc(
		wall.getFullYear(),
		wall.getMonth() + 1,
		wall.getDate(),
		wall.getHours(),
		wall.getMinutes(),
		wall.getSeconds()
	);
	const offset = (t: number) => asUtc(...wallFields(new Date(t))) - t;
	// Once more with the offset found, for a change to or from summer time in between.
	let guess = target - offset(target);
	guess = target - offset(guess);
	return new Date(guess);
}
