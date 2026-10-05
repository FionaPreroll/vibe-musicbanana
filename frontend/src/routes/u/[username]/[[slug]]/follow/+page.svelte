<script lang="ts">
	import { goto } from '$app/navigation';
	import { profilePath, type FollowInfo } from '#lib/api.ts';
	import FollowButton from '#lib/components/FollowButton.svelte';
	import VisibilityBadge from '#lib/components/VisibilityBadge.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const info = $derived(data.info);
	const path = $derived(profilePath(info.username, info.slug));
	// Who may see the profile goes there; everybody else waits for a yes.
	const open = $derived(info.own || info.visibility === 'public' || info.state === 'following');

	function changed(next: FollowInfo) {
		if (next.state === 'following') goto(path);
	}
</script>

<svelte:head>
	<title>{info.username} · musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-2xl px-4 pb-16 sm:px-8">
	<h1 class="mt-6 text-3xl font-bold">
		{info.username}
		{#if info.slug !== 'default'}<span class="font-normal text-stone-500"> / {info.name}</span>{/if}
		<VisibilityBadge visibility={info.visibility} />
	</h1>
	{#if open}
		<p class="mt-4"><a class="underline" href={path}>Go to the profile</a></p>
	{:else}
		<p class="mt-4 text-stone-600">
			{info.username} shows this profile to followers only. It opens for you once {info.username} says
			yes to your request.
		</p>
		<div class="mt-4"><FollowButton {info} onchange={changed} /></div>
	{/if}
</main>
