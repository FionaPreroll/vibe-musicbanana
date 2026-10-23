// The backend answers refusals with an English sentence (see AppError in
// backend/src/lib.rs). These patterns know the ones a person can run into, so
// that a catalog can say them in its language; anything else is shown as sent.

import { m } from './index.ts';

const patterns = {
	notLoggedIn: /^not logged in$/,
	adminsOnly: /^this is for admins$/,
	tooManyLogins: /^too many failed logins; try again in (\d+) minutes$/,
	wrongLogin: /^wrong user name or password$/,
	currentPasswordWrong: /^the current password is wrong$/,
	passwordTooShort: /^the new password needs at least (\d+) characters$/,
	passwordWrong: /^the password is wrong$/,
	profileNeedsName: /^the profile needs a name$/,
	profileExists: /^you have a profile "(.*)" already$/s,
	tokenNeedsLabel: /^name what the token is for, e\.g\. Navidrome$/,
	yourSpotifyCliOnly: /^this server takes YourSpotify connections from its command line only$/,
	yourSpotifyOnly: /^this server takes YourSpotify at (.*) only$/s,
	yourSpotifyConnected: /^the profile is connected to YourSpotify already\b/,
	yourSpotifyTokenEmpty: /^the YourSpotify token is empty$/,
	yourSpotifyTokenRefused: /^YourSpotify at (.*) does not take the token\b/s,
	yourSpotifyAnswered: /^YourSpotify at (.*) answered (.*) when asked for its history$/s,
	yourSpotifyUnreachable: /^asking YourSpotify at (.*?) for its history: (.*)$/s,
	notHttp: /^(.*) is not an http\(s\) address$/s,
	notAnAddress: /^(.*) is not an address: (.*)$/s,
	badEmail: /^"(.*)" does not look like an email address$/s,
	emailTaken: /^another account has the address (.*)$/s,
	badUsername: /^"(.*)" does not work as a user name\b/s,
	usernameTaken: /^the user name "(.*)" is taken$/s,
	badSlug: /^"(.*)" does not work as a profile address\b/s,
	slugReserved: /^"(.*)" is taken by the pages of a profile$/s,
	ownProfile: /^that is your own profile$/,
	compareWithItself: /^a profile cannot be compared with itself$/,
	pickListens: /^pick listens, or narrow the history down first$/,
	unknownTimeZone: /^unknown time zone: (.*)$/s,
	unknownLanguage: /^unknown language: (.*)$/s,
	internal: /^internal server error$/,
	notFound: /^Not Found$/
} satisfies Record<string, RegExp>;

/** What each pattern's message says in a language, from the pattern's groups. */
export type ServerErrors = { [K in keyof typeof patterns]: (...groups: string[]) => string };

/** The backend's message in the page's language, where the catalog has it; else as sent, capitalized. */
export function translateServerError(message: string) {
	const own = m.serverErrors;
	for (const [key, pattern] of Object.entries(patterns) as [keyof typeof patterns, RegExp][]) {
		const match = pattern.exec(message);
		const say = own[key];
		if (match && say) return say(...match.slice(1));
	}
	return message.charAt(0).toUpperCase() + message.slice(1);
}

/** The keys, for the tests: every catalog besides English should know them all. */
export const serverErrorKeys = Object.keys(patterns) as (keyof typeof patterns)[];
