<script lang="ts">
	import type { SharedEntry } from '#lib/api.ts';
	import { formatNumber } from '#lib/format.ts';

	let {
		title,
		entries,
		names,
		href
	}: {
		title: string;
		entries: SharedEntry[];
		/** The two profiles, for the bars' labels. */
		names: [string, string];
		/** Makes each entry a link. */
		href?: (entry: SharedEntry) => string;
	} = $props();

	// Each bar against the side's own most heard entry, as both sides may differ a lot in listens.
	const maxA = $derived(Math.max(1, ...entries.map((e) => e.a)));
	const maxB = $derived(Math.max(1, ...entries.map((e) => e.b)));
</script>

<section>
	<h2 class="mb-2 text-sm font-semibold tracking-wide text-stone-500 uppercase">{title}</h2>
	{#if entries.length === 0}
		<p class="text-sm text-stone-500">Nothing in common.</p>
	{:else}
		<ol class="space-y-1">
			{#each entries as entry, i (entry.id)}
				<li>
					<svelte:element
						this={href ? 'a' : 'div'}
						href={href?.(entry)}
						class="block rounded px-2 py-1 {href ? 'hover:bg-stone-900/5' : ''}"
					>
						<span class="flex items-baseline gap-2">
							<span class="w-5 shrink-0 text-right text-xs text-stone-400 tabular-nums"
								>{i + 1}</span
							>
							<span class="min-w-0 flex-1 truncate" title={entry.name}>
								{entry.name}
								{#if entry.artist}<span class="text-stone-500"> · {entry.artist}</span>{/if}
							</span>
							<span class="text-sm text-stone-600 tabular-nums">
								<span title={names[0]}>{formatNumber(entry.a)}</span>
								<span class="text-stone-400">/</span>
								<span title={names[1]}>{formatNumber(entry.b)}</span>
							</span>
						</span>
						<span class="mt-0.5 ml-7 flex flex-col gap-0.5" aria-hidden="true">
							<span
								class="h-1 rounded-full bg-(--series-1)"
								style:width="{Math.max(2, (entry.a / maxA) * 100)}%"
							></span>
							<span
								class="h-1 rounded-full bg-(--series-3)"
								style:width="{Math.max(2, (entry.b / maxB) * 100)}%"
							></span>
						</span>
					</svelte:element>
				</li>
			{/each}
		</ol>
	{/if}
</section>
