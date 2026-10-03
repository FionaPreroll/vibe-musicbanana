<script lang="ts">
	import type { ChartEntry } from '#lib/api.ts';
	import { formatNumber } from '#lib/format.ts';

	let { title, entries }: { title: string; entries: ChartEntry[] } = $props();

	const max = $derived(entries[0]?.listens ?? 1);
</script>

<section>
	<h2 class="mb-2 text-sm font-semibold tracking-wide text-stone-500 uppercase">{title}</h2>
	{#if entries.length === 0}
		<p class="text-sm text-stone-500">Nothing here.</p>
	{:else}
		<ol class="space-y-0.5">
			{#each entries as entry, i (entry.id)}
				<li class="relative overflow-hidden rounded">
					<div
						class="absolute inset-y-0 left-0 bg-yellow-200/70"
						style:width="{(entry.listens / max) * 100}%"
					></div>
					<div class="relative flex items-baseline gap-2 px-2 py-1">
						<span class="w-5 shrink-0 text-right text-xs text-stone-400 tabular-nums">{i + 1}</span>
						<span class="min-w-0 flex-1 truncate" title={entry.name}>
							{entry.name}
							{#if entry.artist}<span class="text-stone-500"> · {entry.artist}</span>{/if}
						</span>
						<span class="text-sm text-stone-600 tabular-nums">{formatNumber(entry.listens)}</span>
					</div>
				</li>
			{/each}
		</ol>
	{/if}
</section>
