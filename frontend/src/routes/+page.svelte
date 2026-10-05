<script lang="ts">
	import { profilePath } from '#lib/api.ts';
	import VisibilityBadge from '#lib/components/VisibilityBadge.svelte';
	import { listenCount } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<svelte:head>
	<title>musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-2xl px-4 pb-16 sm:px-8">
	<p class="mt-6 text-stone-600">A free place for your listening history.</p>

	{#if data.following.length > 0}
		<h2 class="mt-10 mb-2 text-sm font-semibold tracking-wide text-stone-500 uppercase">
			You follow
		</h2>
		<ul class="divide-y divide-stone-200">
			{#each data.following as f (`${f.username}/${f.slug}`)}
				<li class="flex items-baseline justify-between gap-4 py-2">
					<span>
						<a
							class="font-medium hover:underline"
							href={f.state === 'following'
								? profilePath(f.username, f.slug)
								: `${profilePath(f.username, f.slug)}/follow`}
						>
							{f.username}
							{#if f.slug !== 'default'}<span class="font-normal text-stone-500">
									/ {f.name}</span
								>{/if}
						</a>
						<VisibilityBadge visibility={f.visibility} />
					</span>
					{#if f.state === 'requested'}
						<span class="text-sm text-stone-500">Asked, no answer yet</span>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}

	<h2 class="mt-10 mb-2 text-sm font-semibold tracking-wide text-stone-500 uppercase">
		{data.following.length > 0 ? 'All profiles' : 'Profiles'}
	</h2>
	{#if data.profiles.length === 0}
		<p class="text-sm text-stone-500">No profiles to show yet.</p>
	{:else}
		<ul class="divide-y divide-stone-200">
			{#each data.profiles as profile (`${profile.username}/${profile.slug}`)}
				<li class="flex items-baseline justify-between gap-4 py-2">
					<span>
						<a
							class="font-medium hover:underline"
							href={profilePath(profile.username, profile.slug)}
						>
							{profile.username}
							{#if profile.slug !== 'default'}<span class="font-normal text-stone-500">
									/ {profile.name}</span
								>{/if}
						</a>
						<VisibilityBadge visibility={profile.visibility} />
					</span>
					<span class="text-sm text-stone-500 tabular-nums">{listenCount(profile.listens)}</span>
				</li>
			{/each}
		</ul>
	{/if}
</main>
