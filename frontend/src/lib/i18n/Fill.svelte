<script lang="ts">
	import type { Snippet } from 'svelte';

	// A text with {name} holes, each filled by the snippet of that name, so that
	// a translation can put links and emphasis where its language wants them:
	// <Fill text={m.entity.by}>{#snippet artist()}<a …>…</a>{/snippet}</Fill>
	let { text, ...holes }: { text: string; [hole: string]: Snippet | string } = $props();

	// Odd parts are the names of holes.
	const parts = $derived(text.split(/\{(\w+)\}/));
	const hole = (name: string) => holes[name] as Snippet | undefined;
</script>

{#each parts as part, i (i)}{#if i % 2 === 0}{part}{:else if hole(part)}{@render hole(
			part
		)!()}{:else}{`{${part}}`}{/if}{/each}
