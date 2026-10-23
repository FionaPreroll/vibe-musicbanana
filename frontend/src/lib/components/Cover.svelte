<script lang="ts">
	import { initial } from '#lib/covers.svelte.ts';

	// An album cover, or a banana-yellow tile with the album's first letter where
	// the album has none (no MusicBrainz ID, nothing in the Cover Art Archive).
	let {
		src,
		srcset,
		name,
		alt = '',
		class: className = 'size-8 text-sm'
	}: {
		/** Null for the placeholder right away. */
		src: string | null;
		srcset?: string;
		/** The album's title, for the placeholder's letter. */
		name: string;
		/** Empty when the title stands next to the cover anyway. */
		alt?: string;
		/** The size, e.g. `size-8 text-sm`; the text size is the letter's. */
		class?: string;
	} = $props();

	let failed = $state(false);
	// Another album in the same place (a list that changed) gets its own try.
	$effect(() => {
		void src;
		failed = false;
	});
</script>

{#if src && !failed}
	<img
		{src}
		{srcset}
		{alt}
		loading="lazy"
		decoding="async"
		class="{className} shrink-0 rounded-sm bg-stone-200 object-cover"
		onerror={() => (failed = true)}
	/>
{:else}
	<span
		class="{className} inline-flex shrink-0 items-center justify-center rounded-sm bg-yellow-300 font-semibold text-stone-800 select-none"
		role={alt ? 'img' : undefined}
		aria-label={alt || undefined}
		aria-hidden={alt ? undefined : 'true'}>{initial(name)}</span
	>
{/if}
