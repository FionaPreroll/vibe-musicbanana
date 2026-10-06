<script lang="ts">
	import './layout.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { profilePath, sendJson } from '#lib/api.ts';
	import BananaEgg from '#lib/components/BananaEgg.svelte';
	import ThemeToggle from '#lib/components/ThemeToggle.svelte';
	import favicon from '#lib/assets/favicon.svg';

	let { data, children } = $props();

	let bananaOpen = $state(false);

	async function logOut() {
		await sendJson(fetch, 'DELETE', '/api/session');
		await goto('/', { invalidateAll: true });
	}

	// On the pages of a profile, the header searches that profile.
	const profile = $derived(
		page.params.username ? profilePath(page.params.username, page.params.slug) : null
	);
	const onSearchPage = $derived(page.route.id?.endsWith('/search') ?? false);

	// Back to where the login started, but not to the login page itself.
	const loginHref = $derived(
		page.url.pathname === '/login'
			? '/login'
			: `/login?next=${encodeURIComponent(page.url.pathname + page.url.search)}`
	);
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<header class="border-b border-stone-200 bg-white">
	<div
		class="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-x-4 gap-y-2 px-4 py-3 sm:px-8"
	>
		<div class="shrink-0 text-lg font-bold">
			<a href="/">musicbanana</a>
			<button
				type="button"
				class="cursor-pointer"
				aria-expanded={bananaOpen}
				aria-controls="banana"
				onclick={() => (bananaOpen = !bananaOpen)}>🍌</button
			>
		</div>
		{#if profile && !onSearchPage}
			<form class="min-w-0 flex-1 sm:max-w-xs" method="GET" action="{profile}/search" role="search">
				<input
					class="w-full rounded border border-stone-300 px-2 py-1 text-sm"
					type="search"
					name="q"
					placeholder="Search {page.params.username}"
					aria-label="Search the artists, albums and tracks of this profile"
				/>
				{#if page.url.searchParams.has('source')}
					<input type="hidden" name="source" value={page.url.searchParams.get('source')} />
				{/if}
			</form>
		{/if}
		<nav class="flex shrink-0 items-baseline gap-3 text-sm sm:gap-4">
			{#if data.me}
				<a class="font-medium hover:underline" href="/u/{encodeURIComponent(data.me.username)}"
					>{data.me.username}</a
				>
				{#if data.me.admin}
					<a class="text-stone-600 hover:underline" href="/status">Status</a>
				{/if}
				<a class="text-stone-600 hover:underline" href="/settings">Settings</a>
				<button class="text-stone-600 hover:underline" onclick={logOut}>Log out</button>
			{:else}
				<a class="text-stone-600 hover:underline" href={loginHref}>Log in</a>
			{/if}
			<ThemeToggle />
		</nav>
	</div>
</header>
<BananaEgg bind:open={bananaOpen} />

{@render children()}
