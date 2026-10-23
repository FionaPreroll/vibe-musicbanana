// The catalogs, picking the language and the backend's messages in it.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { errorMessage } from '#lib/api.ts';
import { formatDuration, formatNumber, listenCount, timeSince } from '#lib/format.ts';
import de from './de.ts';
import en from './en.ts';
import { catalogs, language, languageNames, languages, m } from './index.ts';
import { formatList, listParts, plural } from './intl.ts';
import {
	browserLanguage,
	formatLocale,
	formatLocaleFor,
	pickLanguage,
	useLanguage
} from './locale.svelte.ts';
import { serverErrorKeys, translateServerError } from './server-errors.ts';

afterEach(() => {
	vi.unstubAllGlobals();
	useLanguage(null);
});

/** Every key of `shape` with the kind of its value, functions with their arity. */
function outline(shape: unknown, path = ''): string[] {
	if (typeof shape === 'function') return [`${path}: function(${shape.length})`];
	if (Array.isArray(shape)) return shape.flatMap((v, i) => outline(v, `${path}[${i}]`));
	if (shape && typeof shape === 'object') {
		return Object.entries(shape).flatMap(([k, v]) => outline(v, path ? `${path}.${k}` : k));
	}
	return [`${path}: ${typeof shape}`];
}

describe('catalogs', () => {
	it('have the shape of the English one', () => {
		const { serverErrors: _en, ...english } = en;
		for (const code of languages) {
			const { serverErrors: _own, ...own } = catalogs[code];
			expect(outline(own), code).toEqual(outline(english));
		}
	});

	it('know every message of the backend, except English, which shows them as sent', () => {
		expect(Object.keys(en.serverErrors)).toEqual([]);
		for (const code of languages.filter((l) => l !== 'en')) {
			expect(Object.keys(catalogs[code].serverErrors).sort(), code).toEqual(
				[...serverErrorKeys].sort()
			);
		}
	});

	it('fill holes the templates name', () => {
		for (const code of languages) {
			const holes = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((h) => h[1]).sort();
			const c = catalogs[code];
			expect(holes(c.login.help), code).toEqual(['create', 'reset']);
			expect(holes(c.entity.by), code).toEqual(['artist']);
			expect(holes(c.review.mostly), code).toEqual(['artists']);
			expect(holes(c.settings.tokens.created), code).toEqual(['label']);
			expect(holes(c.settings.yourspotify.intro), code).toEqual(['fetchAll', 'yourspotify']);
			expect(holes(c.settings.yourspotify.cliOnly), code).toEqual(['variable']);
			expect(holes(c.settings.followers.asks('x')), code).toEqual(['profile', 'user']);
			expect(holes(c.settings.followers.follows('x')), code).toEqual(['profile', 'user']);
			expect(holes(c.settings.username.renamed('x')), code).toEqual(['link']);
			expect(holes(c.history.intro), code).toEqual(['profile']);
			expect(holes(c.merges.wouldMerge('x')), code).toEqual(['from', 'into']);
			expect(holes(c.merges.logIntro), code).toEqual(['editions']);
		}
	});

	it('name themselves for the language picker', () => {
		expect(languageNames()).toEqual([
			{ code: 'en', name: 'English' },
			{ code: 'de', name: 'Deutsch' }
		]);
	});
});

describe('picking the language', () => {
	it('takes the first of the browser languages it speaks', () => {
		expect(pickLanguage(['fr-FR', 'de-AT', 'en'])).toBe('de');
		expect(pickLanguage(['EN-gb'])).toBe('en');
		expect(pickLanguage(['fr', 'it'])).toBeNull();
		expect(pickLanguage([])).toBeNull();
	});

	it('formats in the browser’s own variant of the language', () => {
		expect(formatLocaleFor('de', ['fr-FR', 'de-AT'])).toBe('de-AT');
		expect(formatLocaleFor('de', ['en-GB'])).toBe('de');
		expect(formatLocaleFor('en', ['en-gb'])).toBe('en-GB');
		expect(formatLocaleFor('en', ['en-x-'])).toBe('en');
	});

	it('follows the browser, else English', () => {
		vi.stubGlobal('navigator', { languages: ['de-CH', 'en'] });
		expect(browserLanguage()).toBe('de');
		useLanguage(null);
		expect([language(), formatLocale()]).toEqual(['de', 'de-CH']);
		expect(document.documentElement.lang).toBe('de');

		vi.stubGlobal('navigator', { languages: ['ja-JP'] });
		useLanguage(null);
		expect([language(), formatLocale()]).toEqual(['en', 'en']);
	});

	it('takes the account’s pick over the browser’s, unless it does not speak it', () => {
		vi.stubGlobal('navigator', { languages: ['en-GB'] });
		useLanguage('de');
		expect([language(), formatLocale()]).toEqual(['de', 'de']);
		expect(m.settings.title).toBe('Einstellungen');
		useLanguage('xx');
		expect(language()).toBe('en');
		expect(m.settings.title).toBe('Settings');
	});
});

describe('in German', () => {
	it('counts, formats and says plurals', () => {
		useLanguage('de');
		expect(listenCount(1)).toBe('1 Wiedergabe');
		expect(listenCount(1234)).toBe('1.234 Wiedergaben');
		expect(m.days(1)).toBe('1 Tag');
		expect(m.days(2)).toBe('2 Tage');
		expect(formatNumber(1234567.5)).toBe('1.234.567,5');
		expect(formatDuration(3 * 3_600_000 + 12 * 60_000)).toBe('3 Std. 12 Min.');
		expect(formatDuration(2 * 3_600_000)).toBe('2 Std.');
		expect(formatDuration(45 * 60_000)).toBe('45 Min.');
		expect(formatList(['Die Ärzte', 'Björk', 'Tocotronic'])).toBe(
			'Die Ärzte, Björk und Tocotronic'
		);
		expect(plural(1, { one: 'a', other: 'b' })).toBe('a');
		expect(plural(0, { other: 'b' })).toBe('b');
	});

	it('says how long since in the dative, after „seit“', () => {
		useLanguage('de');
		const since = new Date('2007-08-12T14:28:00Z');
		expect(timeSince(since, new Date('2026-09-20T14:28:00Z'))).toBe('19 Jahren und 1 Monat');
		expect(timeSince(since, new Date('2007-08-12T15:28:00Z'))).toBe('1 Stunde');
		expect(timeSince(since, since)).toBe('0 Minuten');
		expect(m.banana.lives('19 Jahren')).toBe('musicbanana lebt schon seit 19 Jahren');
	});

	it('compares weeks and years in whole sentences', () => {
		useLanguage('de');
		expect(m.week.compare(-21, 56, true)).toBe('21 weniger als letzte Woche um diese Zeit');
		expect(m.week.compare(3, 5, false)).toBe('3 mehr als letzte Woche');
		expect(m.week.compare(0, 5, false)).toBe('so viele wie letzte Woche');
		expect(m.week.compare(4, 0, true)).toBe('letzte Woche um diese Zeit noch keine');
		expect(m.week.compare(4, 0, false)).toBe('letzte Woche keine');
		expect(m.review.compare(0.03, 10, 2024, false, '3 %')).toBe('3 % mehr als 2024');
		expect(m.review.compare(-0.1, 10, 2025, true, '10 %')).toBe(
			'10 % weniger als im gleichen Zeitraum 2025'
		);
		expect(m.review.compare(0, 10, 2025, true, '0 %')).toBe(
			'so viele wie im gleichen Zeitraum 2025'
		);
		expect(m.review.compare(0, 0, 2025, true, '0 %')).toBe('2025 im gleichen Zeitraum keine');
		expect(m.review.compare(0, 0, 2025, false, '0 %')).toBe('2025 keine');
		expect(m.history.total(1, true)).toBe('1 Wiedergabe passt.');
		expect(m.history.total(2, true)).toBe('2 Wiedergaben passen.');
		expect(m.history.confirmMoveAll(1)).toBe(
			'Die passende Wiedergabe in den Papierkorb verschieben?'
		);
		expect(m.history.restored(3, 1)).toBe(
			'3 Wiedergaben wiederhergestellt. 1 Wiedergabe bleibt im Papierkorb, weil das Profil zu dieser Zeit inzwischen eine andere Wiedergabe hat.'
		);
		expect(m.merges.wouldUndo(7, 'Künstler', 'Die Arzte', 1, 2)).toBe(
			'Würde Zusammenführung 7 rückgängig machen: Künstler „Die Arzte“ steht wieder für sich, 1 Änderung wird zurückgenommen. 2 Zeilen bleiben, wie sie sind, weil sie sich seitdem wieder geändert haben.'
		);
	});
});

describe('lists of links', () => {
	it('split into the items and the words between them', () => {
		useLanguage('de');
		expect(listParts(3)).toEqual([
			{ index: 0, text: '' },
			{ index: -1, text: ', ' },
			{ index: 1, text: '' },
			{ index: -1, text: ' und ' },
			{ index: 2, text: '' }
		]);
		useLanguage('en');
		expect(listParts(2).map((p) => p.text || p.index)).toEqual([0, ' and ', 1]);
		expect(listParts(1)).toEqual([{ index: 0, text: '' }]);
	});
});

describe('the backend’s messages', () => {
	// As the backend words them (backend/src).
	const sent: [string, string][] = [
		['not logged in', 'Nicht angemeldet.'],
		['this is for admins', 'Das ist nur für Admins.'],
		[
			'too many failed logins; try again in 15 minutes',
			'Zu viele fehlgeschlagene Anmeldungen. Versuch es in 15 Minuten noch einmal.'
		],
		['wrong user name or password', 'Benutzername oder Passwort stimmt nicht.'],
		['the current password is wrong', 'Das aktuelle Passwort stimmt nicht.'],
		[
			'the new password needs at least 8 characters',
			'Das neue Passwort braucht mindestens 8 Zeichen.'
		],
		['the password is wrong', 'Das Passwort stimmt nicht.'],
		['the profile needs a name', 'Das Profil braucht einen Namen.'],
		['you have a profile "spotify" already', 'Du hast schon ein Profil „spotify“.'],
		[
			'name what the token is for, e.g. Navidrome',
			'Gib an, wofür das Token ist, zum Beispiel Navidrome.'
		],
		[
			'this server takes YourSpotify connections from its command line only',
			'Dieser Server nimmt YourSpotify-Verbindungen nur über die Kommandozeile an.'
		],
		[
			'this server takes YourSpotify at http://a:8080 or http://b only',
			'Dieser Server nimmt YourSpotify nur unter http://a:8080 oder http://b an.'
		],
		[
			'the profile is connected to YourSpotify already (connection 3); remove that one first',
			'Das Profil ist schon mit YourSpotify verbunden. Entferne zuerst die bestehende Verbindung.'
		],
		['the YourSpotify token is empty', 'Das YourSpotify-Token fehlt.'],
		[
			"YourSpotify at http://ys does not take the token; the public token is in YourSpotify's settings",
			'YourSpotify unter http://ys nimmt das Token nicht an. Das öffentliche Token steht in den Einstellungen von YourSpotify.'
		],
		[
			'YourSpotify at http://ys answered 502 Bad Gateway when asked for its history',
			'YourSpotify unter http://ys hat auf die Frage nach dem Verlauf mit 502 Bad Gateway geantwortet.'
		],
		[
			'asking YourSpotify at http://ys for its history: error sending request',
			'YourSpotify unter http://ys ist nicht erreichbar: error sending request'
		],
		['ftp://ys is not an http(s) address', 'ftp://ys ist keine http(s)-Adresse.'],
		['ys is not an address: relative URL without a base', 'ys ist keine gültige Adresse.'],
		[
			'"fiona" does not look like an email address',
			'„fiona“ sieht nicht nach einer E-Mail-Adresse aus.'
		],
		[
			'another account has the address alex@example.org',
			'Ein anderes Konto hat schon die Adresse alex@example.org.'
		],
		[
			'"-x" does not work as a user name: use up to 32 letters, digits, dashes, dots and underscores, starting with a letter or digit',
			'„-x“ geht nicht als Benutzername: erlaubt sind bis zu 32 Buchstaben, Ziffern, Bindestriche, Punkte und Unterstriche, am Anfang ein Buchstabe oder eine Ziffer.'
		],
		['the user name "alex" is taken', 'Der Benutzername „alex“ ist schon vergeben.'],
		[
			'"A B" does not work as a profile address: use up to 32 lower-case letters, digits and dashes',
			'„A B“ geht nicht als Profiladresse: erlaubt sind bis zu 32 Kleinbuchstaben, Ziffern und Bindestriche.'
		],
		[
			'"artist" is taken by the pages of a profile',
			'„artist“ ist für die Seiten eines Profils reserviert.'
		],
		['that is your own profile', 'Das ist dein eigenes Profil.'],
		[
			'pick listens, or narrow the history down first',
			'Wähle Wiedergaben aus oder grenze den Verlauf zuerst ein.'
		],
		['unknown time zone: Mars/Olympus', 'Unbekannte Zeitzone: Mars/Olympus'],
		['unknown language: deutsch', 'Unbekannte Sprache: deutsch'],
		['internal server error', 'Interner Serverfehler.'],
		['Not Found', 'Nicht gefunden.']
	];

	it('are said in German', () => {
		useLanguage('de');
		for (const [message, german] of sent) expect(translateServerError(message)).toBe(german);
		expect(errorMessage(new Error('wrong user name or password'))).toBe(
			'Benutzername oder Passwort stimmt nicht.'
		);
		// One it does not know stays as sent.
		expect(translateServerError('merge 3 was undone already')).toBe('Merge 3 was undone already');
	});

	it('cover every pattern', () => {
		useLanguage('de');
		const said = new Set(sent.map(([, german]) => german));
		expect(said.size).toBe(serverErrorKeys.length);
	});

	it('stay as sent in English, capitalized', () => {
		useLanguage('en');
		expect(errorMessage(new Error('wrong user name or password'))).toBe(
			'Wrong user name or password'
		);
		expect(errorMessage('not logged in')).toBe('Not logged in');
	});
});

describe('German texts', () => {
	it('come out of every function without English words', () => {
		// The values a text gets are stand-ins; what is left must be German.
		const english =
			/\b(the|and|of|with|listens?|tracks?|artists?|ago|since|days?|none|more|fewer)\b/;
		const texts = outline(de)
			.filter((line) => !line.startsWith('serverErrors'))
			.map((line) => line.split(':')[0]);
		const get = (path: string) =>
			path
				.split(/\.|\[(\d+)\]/)
				.filter(Boolean)
				.reduce<unknown>((value, key) => (value as Record<string, unknown>)[key], de);
		useLanguage('de');
		for (const path of texts) {
			const value = get(path);
			const text =
				typeof value === 'function'
					? String((value as (...a: unknown[]) => unknown)(2, 3, 2020, true, 'X'))
					: String(value);
			const exempt = [
				'languageName',
				'merges.likeness.same_letters[1]',
				'merges.likeness.version[1]',
				'search.hint'
			];
			if (exempt.includes(path)) continue;
			expect(text.replace(/\{\w+\}/g, ''), path).not.toMatch(english);
		}
	});
});
