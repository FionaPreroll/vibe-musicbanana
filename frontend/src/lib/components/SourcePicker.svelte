<script lang="ts">
	import { page } from '$app/state';
	import type { Source } from '#lib/api.ts';
	import { formatDate, listenCount } from '#lib/format.ts';
	import { sourceLabel, sourceOf } from '#lib/source.ts';

	// Narrows the page to the listens of one source, keeping the rest of the query.
	let { sources }: { sources: Source[] } = $props();

	const current = $derived(sourceOf(page.url));
	function href(source: string | null) {
		const params = new URLSearchParams(page.url.search);
		if (source === null) params.delete('source');
		else params.set('source', source);
		return params.size ? `${page.url.pathname}?${params}` : page.url.pathname;
	}
	const button = (selected: boolean) =>
		`shrink-0 rounded px-2 py-1 text-sm whitespace-nowrap ${
			selected ? 'bg-stone-900 text-white' : 'text-stone-600 hover:bg-stone-200'
		}`;
</script>

<!-- Only worth showing when there is something to choose, or a choice to undo. -->
{#if sources.length > 1 || current !== null}
	<nav class="mt-3 flex flex-wrap items-baseline gap-1" aria-label="Source">
		<span class="mr-1 text-sm text-stone-500">From</span>
		<a
			href={href(null)}
			data-sveltekit-noscroll
			class={button(current === null)}
			aria-current={current === null ? 'page' : undefined}>All sources</a
		>
		{#each sources as s (s.source)}
			<a
				href={href(s.source)}
				data-sveltekit-noscroll
				class={button(current === s.source)}
				aria-current={current === s.source ? 'page' : undefined}
				title="{listenCount(s.listens)}, {formatDate(s.first_listened_at)} to {formatDate(
					s.last_listened_at
				)}">{sourceLabel(s.source)}</a
			>
		{/each}
	</nav>
{/if}
