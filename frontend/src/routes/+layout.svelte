<script lang="ts">
	import './layout.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { profilePath, sendJson } from '#lib/api.ts';
	import favicon from '#lib/assets/favicon.svg';

	let { data, children } = $props();

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
	<div class="mx-auto flex max-w-6xl items-center justify-between gap-4 px-4 py-3 sm:px-8">
		<a href="/" class="shrink-0 text-lg font-bold">musicbanana 🍌</a>
		{#if profile && !onSearchPage}
			<form class="min-w-0 flex-1 sm:max-w-xs" method="GET" action="{profile}/search" role="search">
				<input
					class="w-full rounded border border-stone-300 px-2 py-1 text-sm"
					type="search"
					name="q"
					placeholder="Search {page.params.username}"
					aria-label="Search the artists, albums and tracks of this profile"
				/>
			</form>
		{/if}
		<nav class="flex shrink-0 items-baseline gap-4 text-sm">
			{#if data.me}
				<a class="font-medium hover:underline" href="/u/{encodeURIComponent(data.me.username)}"
					>{data.me.username}</a
				>
				<a class="text-stone-600 hover:underline" href="/settings">Settings</a>
				<button class="text-stone-600 hover:underline" onclick={logOut}>Log out</button>
			{:else}
				<a class="text-stone-600 hover:underline" href={loginHref}>Log in</a>
			{/if}
		</nav>
	</div>
</header>

{@render children()}
