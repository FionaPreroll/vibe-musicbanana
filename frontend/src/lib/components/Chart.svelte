<script lang="ts">
	import type { ChartEntry } from '#lib/api.ts';
	import { formatNumber } from '#lib/format.ts';
	import Cover from '#lib/components/Cover.svelte';
	import { movement, movementText as text } from '#lib/movement.ts';

	let {
		title,
		entries,
		href,
		hideArtist,
		cover,
		more,
		pageSize = 20
	}: {
		title: string;
		entries: ChartEntry[];
		/** Makes each entry a link. */
		href?: (entry: ChartEntry) => string;
		/** Leaves out this artist's name, e.g. on the artist's own page. */
		hideArtist?: string;
		/** Album covers before the names, see covers.svelte.ts. */
		cover?: (entry: ChartEntry) => string | null;
		/** Loads the next places from `offset` on; adds "Show more" below the chart. */
		more?: (offset: number, limit: number) => Promise<ChartEntry[]>;
		/** How many places "Show more" adds. */
		pageSize?: number;
	} = $props();

	// The places loaded with "Show more", dropped when the chart gets new entries
	// (another period or profile).
	let extra: ChartEntry[] = $state([]);
	let loading = $state(false);
	let failed = $state(false);
	let exhausted = $state(false);
	let generation = 0;
	$effect.pre(() => {
		void entries;
		generation++;
		extra = [];
		loading = false;
		failed = false;
		exhausted = false;
	});

	const shown = $derived(extra.length ? [...entries, ...extra] : entries);
	const max = $derived(entries[0]?.listens ?? 1);
	// A chart shorter than its first page has nothing more to show.
	const canLoad = $derived(!!more && !exhausted && entries.length >= 10);

	async function loadMore() {
		if (!more || loading) return;
		const asked = generation;
		loading = true;
		failed = false;
		try {
			const next = await more(shown.length, pageSize);
			if (asked !== generation) return;
			// Another listen can shift the places between two pages; keep each entry once.
			const seen = new Set(shown.map((e) => e.id));
			extra = [...extra, ...next.filter((e) => !seen.has(e.id))];
			if (next.length < pageSize) exhausted = true;
		} catch {
			if (asked === generation) failed = true;
		} finally {
			if (asked === generation) loading = false;
		}
	}

	function showLess() {
		generation++;
		extra = [];
		loading = false;
		failed = false;
		exhausted = false;
	}
</script>

<section class="min-w-0">
	<h2 class="mb-2 text-sm font-semibold tracking-wide text-stone-500 uppercase">{title}</h2>
	{#if entries.length === 0}
		<p class="text-sm text-stone-500">{text.empty}</p>
	{:else}
		<ol class="space-y-0.5">
			{#each shown as entry, i (entry.id)}
				{@const move = movement(entry)}
				<li class="relative overflow-hidden rounded">
					<div
						class="absolute inset-y-0 left-0 bg-yellow-200/70"
						style:width="{Math.min(entry.listens / max, 1) * 100}%"
					></div>
					<svelte:element
						this={href ? 'a' : 'div'}
						href={href?.(entry)}
						class="relative flex {cover ? 'items-center' : 'items-baseline'} gap-2 px-2 py-1 {href
							? 'hover:bg-stone-900/5'
							: ''}"
					>
						<span class="w-6 shrink-0 text-right text-xs text-stone-400 tabular-nums"
							>{entry.rank ?? i + 1}</span
						>
						{#if cover}
							<Cover src={cover(entry)} name={entry.name} class="size-7 text-xs" />
						{/if}
						<span class="min-w-0 flex-1 truncate" title={entry.name}>
							{entry.name}
							{#if entry.artist && entry.artist !== hideArtist}<span class="text-stone-500">
									· {entry.artist}</span
								>{/if}
						</span>
						{#if move}
							<span
								class="w-8 shrink-0 text-right text-xs tabular-nums"
								data-movement={move.kind}
								title={move.kind === 'new'
									? text.newTitle
									: move.kind === 'same'
										? text.sameTitle
										: move.kind === 'up'
											? text.upTitle(move.places, entry.previous_rank ?? 0)
											: text.downTitle(move.places, entry.previous_rank ?? 0)}
							>
								{#if move.kind === 'up'}
									<span class="text-green-800">↑{move.places}</span>
								{:else if move.kind === 'down'}
									<span class="text-red-700">↓{move.places}</span>
								{:else if move.kind === 'new'}
									<span
										class="rounded bg-yellow-300/70 px-1 text-[0.65rem] font-semibold tracking-wide text-stone-900 uppercase"
										>{text.new}</span
									>
								{:else}
									<span class="text-stone-400">{text.same}</span>
								{/if}
							</span>
						{/if}
						<span class="text-sm text-stone-600 tabular-nums">{formatNumber(entry.listens)}</span>
					</svelte:element>
				</li>
			{/each}
		</ol>
		{#if canLoad || extra.length}
			<div class="mt-2 flex flex-wrap items-baseline gap-3 px-2 text-sm">
				{#if canLoad}
					<button
						type="button"
						class="font-medium text-stone-600 hover:text-stone-900 disabled:opacity-60"
						disabled={loading}
						onclick={loadMore}>{loading ? text.loading : text.showMore}</button
					>
				{/if}
				{#if extra.length}
					<button type="button" class="text-stone-500 hover:text-stone-900" onclick={showLess}
						>{text.showLess}</button
					>
				{/if}
				{#if failed}<span class="text-red-700">{text.failed}</span>{/if}
			</div>
		{/if}
	{/if}
</section>
