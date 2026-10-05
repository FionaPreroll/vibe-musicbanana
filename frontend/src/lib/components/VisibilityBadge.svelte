<script lang="ts">
	import type { Visibility } from '#lib/api.ts';

	// Marks a profile that not everybody can see; public ones get nothing.
	let { visibility }: { visibility: Visibility } = $props();
</script>

{#if visibility !== 'public'}
	<span
		class="inline-block rounded-full border border-stone-300 px-2 py-0.5 align-middle text-xs font-medium text-stone-600"
		title={visibility === 'private'
			? 'Only you see this profile'
			: 'You and your followers see this profile'}
	>
		{visibility === 'private' ? 'Private' : 'Followers only'}
	</span>
{/if}
