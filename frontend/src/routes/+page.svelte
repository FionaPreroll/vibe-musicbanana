<script lang="ts">
	import { listenCount } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<svelte:head>
	<title>musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-2xl px-4 pb-16 sm:px-8">
	<p class="mt-6 text-stone-600">A free place for your listening history.</p>

	<h2 class="mt-10 mb-2 text-sm font-semibold tracking-wide text-stone-500 uppercase">Profiles</h2>
	{#if data.profiles.length === 0}
		<p class="text-sm text-stone-500">No public profiles yet.</p>
	{:else}
		<ul class="divide-y divide-stone-200">
			{#each data.profiles as profile (`${profile.username}/${profile.slug}`)}
				<li class="flex items-baseline justify-between gap-4 py-2">
					<a
						class="font-medium hover:underline"
						href="/u/{profile.username}{profile.slug === 'default' ? '' : `/${profile.slug}`}"
					>
						{profile.username}
						{#if profile.slug !== 'default'}<span class="font-normal text-stone-500">
								/ {profile.name}</span
							>{/if}
					</a>
					<span class="text-sm text-stone-500 tabular-nums">{listenCount(profile.listens)}</span>
				</li>
			{/each}
		</ul>
	{/if}
</main>
