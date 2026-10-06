<script lang="ts">
	import {
		errorMessage,
		getJson,
		sendJson,
		type EntityKind,
		type LostAndFound,
		type LostEntry,
		type LostHidden,
		type LostKind
	} from '#lib/api.ts';
	import { formatDate, formatNumber, listenCount, timeAgo } from '#lib/format.ts';
	import { now } from '#lib/now.svelte.ts';

	let {
		lost: loaded,
		api,
		own,
		source,
		href
	}: {
		lost: LostAndFound;
		/** The profile's API, see `profileApi`. */
		api: string;
		/** The slug when the profile is the viewer's own, who may dismiss entries. */
		own: string | null;
		source: string | null;
		/** The page of an artist or track. */
		href: (kind: EntityKind, id: number, name: string) => string;
	} = $props();

	// What the server sent last; dismissing loads it again, so the next one moves up.
	let lost = $derived(loaded);
	// One list's worth at first, so the rest of the page stays close.
	const shown = 5;
	let all = $state(false);
	const artists = $derived(all ? lost.artists : lost.artists.slice(0, shown));
	const tracks = $derived(all ? lost.tracks : lost.tracks.slice(0, shown));
	const more = $derived(lost.artists.length > shown || lost.tracks.length > shown);

	// The last one dismissed, to take it back right away.
	let undo: { kind: LostKind; id: number; name: string } | null = $state(null);
	let hidden: LostHidden[] | null = $state(null);
	let showHidden = $state(false);
	let error = $state('');

	const me = $derived(own && `/api/me/profiles/${encodeURIComponent(own)}/lost-and-found/hidden`);

	function years(entry: LostEntry) {
		const first = new Date(entry.first_listened_at).getFullYear();
		const last = new Date(entry.last_listened_at).getFullYear();
		return first === last ? String(first) : `${first}–${last}`;
	}

	async function reload() {
		lost = await getJson<LostAndFound>(fetch, `${api}/lost-and-found`, { limit: 10, source });
		if (showHidden && me) hidden = await getJson<LostHidden[]>(fetch, me);
	}

	async function change(action: () => Promise<unknown>) {
		error = '';
		try {
			await action();
		} catch (e) {
			error = errorMessage(e);
		}
		try {
			await reload();
		} catch {
			// The list stays as it is until the next try.
		}
	}

	function dismiss(kind: LostKind, entry: LostEntry) {
		if (!me) return;
		// Gone at once; the reload fills the gap.
		lost = {
			...lost,
			artists: kind === 'artist' ? lost.artists.filter((e) => e.id !== entry.id) : lost.artists,
			tracks:
				kind === 'recording'
					? lost.tracks.filter((e) => e.id !== entry.id)
					: lost.tracks.filter((e) => e.artist !== entry.name)
		};
		undo = { kind, id: entry.id, name: entry.name };
		return change(() => sendJson(fetch, 'POST', me, { kind, id: entry.id }));
	}

	function showAgain(kind: LostKind, id: number) {
		if (!me) return;
		if (undo?.kind === kind && undo.id === id) undo = null;
		return change(() => sendJson(fetch, 'DELETE', `${me}/${kind}/${id}`));
	}

	async function toggleHidden() {
		showHidden = !showHidden;
		if (showHidden && me) {
			try {
				hidden = await getJson<LostHidden[]>(fetch, me);
			} catch (e) {
				error = errorMessage(e);
			}
		}
	}
</script>

{#snippet list(title: string, kind: LostKind, entries: LostEntry[])}
	<div>
		<h3 class="text-sm font-semibold text-stone-700">{title}</h3>
		{#if entries.length === 0}
			<p class="mt-1 text-sm text-stone-500">Nothing lost right now.</p>
		{:else}
			<ol class="mt-1 divide-y divide-stone-200">
				{#each entries as entry (entry.id)}
					<li class="flex items-center gap-2 py-1.5">
						<div class="min-w-0 flex-1">
							<p class="truncate">
								<a
									class="font-medium hover:underline"
									href={href(kind === 'artist' ? 'artist' : 'track', entry.id, entry.name)}
									>{entry.name}</a
								>
								{#if entry.artist}<span class="text-stone-500"> · {entry.artist}</span>{/if}
							</p>
							<p
								class="truncate text-sm text-stone-500"
								title="Last heard {formatDate(entry.last_listened_at)}"
							>
								{listenCount(entry.listens)} · {years(entry)} · last heard {timeAgo(
									new Date(entry.last_listened_at),
									now()
								)}
							</p>
						</div>
						{#if me}
							<button
								class="shrink-0 rounded px-2 py-1 text-stone-400 hover:bg-stone-200 hover:text-stone-800"
								title="Not interested any more"
								aria-label="Dismiss {entry.name}"
								onclick={() => dismiss(kind, entry)}>✕</button
							>
						{/if}
					</li>
				{/each}
			</ol>
		{/if}
	</div>
{/snippet}

<section class="mt-12">
	<h2 class="text-sm font-semibold tracking-wide text-stone-500 uppercase">Lost &amp; found</h2>
	<p class="text-sm text-stone-500">
		Heard a lot, but not since {lost.since ? formatDate(lost.since) : 'a long time'}
	</p>

	{#if undo}
		<p class="mt-3 flex flex-wrap items-baseline gap-x-3 rounded bg-yellow-100 px-3 py-2 text-sm">
			<span>Dismissed {undo.name}{undo.kind === 'artist' ? ' and its tracks' : ''}.</span>
			<button
				class="font-medium underline hover:no-underline"
				onclick={() => undo && showAgain(undo.kind, undo.id)}>Undo</button
			>
		</p>
	{/if}
	{#if error}
		<p class="mt-3 text-sm text-red-700">{error}</p>
	{/if}

	<div class="mt-3 grid gap-x-8 gap-y-6 md:grid-cols-2">
		{@render list('Artists', 'artist', artists)}
		{@render list('Tracks', 'recording', tracks)}
	</div>

	<div class="mt-3 flex flex-wrap gap-2">
		{#if more}
			<button
				class="rounded border border-stone-300 px-3 py-1.5 text-sm hover:bg-stone-100"
				onclick={() => (all = !all)}
			>
				{all ? 'Fewer' : 'More'}
			</button>
		{/if}
		{#if me && lost.hidden}
			<button
				class="rounded border border-stone-300 px-3 py-1.5 text-sm hover:bg-stone-100"
				aria-expanded={showHidden}
				onclick={toggleHidden}
			>
				{showHidden ? 'Hide dismissed' : `Dismissed (${formatNumber(lost.hidden)})`}
			</button>
		{/if}
	</div>

	{#if showHidden && hidden}
		<ol class="mt-3 divide-y divide-stone-200 rounded-lg border border-stone-200 px-3">
			{#each hidden as entry (`${entry.kind}-${entry.id}`)}
				<li class="flex items-baseline gap-2 py-1.5">
					<span class="min-w-0 flex-1 truncate">
						<span class="font-medium">{entry.name}</span>
						<span class="text-stone-500">
							{#if entry.artist}· {entry.artist}{:else}· artist and its tracks{/if}
						</span>
					</span>
					<span class="hidden shrink-0 text-sm text-stone-500 sm:inline"
						>{formatDate(entry.hidden_at)}</span
					>
					<button
						class="shrink-0 rounded border border-stone-300 px-2 py-0.5 text-sm hover:bg-stone-100"
						onclick={() => showAgain(entry.kind, entry.id)}>Show again</button
					>
				</li>
			{:else}
				<li class="py-1.5 text-sm text-stone-500">Nothing dismissed.</li>
			{/each}
		</ol>
	{/if}
</section>
