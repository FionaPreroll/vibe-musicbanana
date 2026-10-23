<script lang="ts">
	import type { ProfileSummary } from '#lib/api.ts';
	import VisibilityBadge from '#lib/components/VisibilityBadge.svelte';
	import { comparePath } from '#lib/compare.ts';
	import { listenCount } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const overview = $derived(data.overview);
</script>

<svelte:head>
	<title>Compare · {overview.username} · musicbanana</title>
</svelte:head>

{#snippet choices(title: string, profiles: ProfileSummary[])}
	<section class="mt-6">
		<h2 class="mb-2 text-sm font-semibold tracking-wide text-stone-500 uppercase">{title}</h2>
		<ul class="divide-y divide-stone-200">
			{#each profiles as profile (`${profile.username}/${profile.slug}`)}
				<li>
					<a
						class="flex items-baseline gap-2 px-2 py-2 hover:bg-stone-900/5"
						href={comparePath(data.base, profile.username, profile.slug)}
					>
						<span class="font-medium">{profile.username}</span>
						{#if profile.slug !== 'default'}<span class="text-stone-500">/ {profile.name}</span
							>{/if}
						<VisibilityBadge visibility={profile.visibility} />
						<span class="ml-auto text-sm text-stone-500 tabular-nums"
							>{listenCount(profile.listens)}</span
						>
					</a>
				</li>
			{/each}
		</ul>
	</section>
{/snippet}

<main class="mx-auto max-w-3xl px-4 pb-16 sm:px-8">
	<p class="mt-6 text-sm text-stone-500">
		<a class="hover:text-stone-900 hover:underline" href={data.base}
			>{overview.username}{#if overview.slug !== 'default'}&nbsp;/ {overview.name}{/if}</a
		>
		· Compare
	</p>
	<h1 class="mt-2 text-3xl font-bold">Compare with…</h1>
	<p class="mt-1 text-stone-600">
		See what two profiles both listen to, how much their taste overlaps, and what each one could
		discover in the other.
	</p>

	{#if data.own.length > 0}
		{@render choices('Your profiles', data.own)}
	{/if}
	{#if data.others.length > 0}
		{@render choices(data.own.length > 0 ? 'Other profiles' : 'Profiles', data.others)}
	{/if}
	{#if data.own.length + data.others.length === 0}
		<p class="mt-6 text-sm text-stone-500">
			There is no other profile with listens to compare with.
		</p>
	{/if}
</main>
