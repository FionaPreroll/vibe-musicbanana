// Intl formatters for the current locale (and time zone), made again only when
// that changes. Reading one in a component makes it follow the language.

import { formatLocale, language } from './locale.svelte.ts';

/** A thing made for the current locale, made once per locale. */
export function localized<T>(make: (locale: string) => T): () => T {
	let made: { locale: string; value: T } | undefined;
	return () => {
		const locale = formatLocale();
		if (made?.locale !== locale) made = { locale, value: make(locale) };
		return made.value;
	};
}

const pluralRules = localized(() => new Intl.PluralRules(language()));

/** The plural category of `n` in the page's language: "one", "few", "other" … */
export const pluralOf = (n: number) => pluralRules().select(n);

/** Picks the form for `n`; `other` is the fallback for categories left out. */
export const plural = (
	n: number,
	forms: Partial<Record<Intl.LDMLPluralRule, string>> & { other: string }
) => forms[pluralOf(n)] ?? forms.other;

const numbers = localized((locale) => new Intl.NumberFormat(locale));

/** A number in the current locale, e.g. "1,234" or "1.234". */
export const formatNumber = (n: number) => numbers().format(n);

const lists = localized((locale) => new Intl.ListFormat(locale, { type: 'conjunction' }));

/** "A, B and C" in the current locale. */
export const formatList = (items: string[]) => lists().format(items);

/** The parts of "A, B and C", for lists of links: elements by their index in `items`, and the words between. */
export function listParts(count: number) {
	const marks = Array.from({ length: count }, (_, i) => `\u0000${i}\u0000`);
	return lists()
		.formatToParts(marks)
		.map((part) =>
			part.type === 'element'
				? { index: Number(part.value.replaceAll('\u0000', '')), text: '' }
				: { index: -1, text: part.value }
		);
}
