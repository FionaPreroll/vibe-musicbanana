// The source the pages of a profile are narrowed to, as ?source=Navidrome: where
// the listens came from (a player, an import), see GET …/sources.

/** The source in the URL, or null for all of them. */
export const sourceOf = (url: { searchParams: { get(name: string): string | null } }) =>
	url.searchParams.get('source');

/** A query string with the source added: ('?year=2012', 'Navidrome') → '?year=2012&source=Navidrome'. */
export function withSource(search: string, source: string | null) {
	if (source === null) return search;
	const params = new URLSearchParams(search);
	params.set('source', source);
	return `?${params}`;
}

/** How a source reads on the page. */
export function sourceLabel(source: string) {
	if (source === '') return 'Unknown player';
	if (source === 'Spotify via YourSpotify') return 'Spotify';
	if (source.startsWith('import:php')) return 'Old musicbanana';
	return source;
}
