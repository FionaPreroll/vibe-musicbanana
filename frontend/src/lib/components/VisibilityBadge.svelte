<script lang="ts">
	import type { Visibility } from '#lib/api.ts';
	import { m } from '#lib/i18n/index.ts';

	// Marks a profile that not everybody can see; public ones get nothing.
	let { visibility }: { visibility: Visibility } = $props();
</script>

{#if visibility !== 'public'}
	<span
		class="inline-block rounded-full border border-stone-300 px-2 py-0.5 align-middle text-xs font-medium text-stone-600"
		title={visibility === 'private' ? m.visibility.privateTitle : m.visibility.followersTitle}
	>
		{visibility === 'private' ? m.visibility.private : m.visibility.followers}
	</span>
{/if}
