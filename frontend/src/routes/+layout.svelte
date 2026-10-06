<script lang="ts">
	import './layout.css';
	import { afterNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import { profilePath, sendJson } from '#lib/api.ts';
	import BananaEgg from '#lib/components/BananaEgg.svelte';
	import favicon from '#lib/assets/favicon.svg';

	let { data, children } = $props();

	let bananaOpen = $state(false);

	// On phones the account links fold into a menu.
	let menuOpen = $state(false);
	let menu: HTMLElement | undefined = $state();
	afterNavigate(() => (menuOpen = false));

	function closeMenuOutside(event: MouseEvent) {
		if (menuOpen && !menu?.contains(event.target as Node)) menuOpen = false;
	}

	// `/` jumps into the search of the page (the header's, or the search page's own),
	// unless the key is meant for a field.
	function keydown(event: KeyboardEvent) {
		if (event.key === 'Escape') menuOpen = false;
		if (event.key !== '/' || event.ctrlKey || event.metaKey || event.altKey) return;
		const target = event.target as HTMLElement | null;
		if (target?.closest('input, textarea, select, [contenteditable]')) return;
		const search = document.querySelector<HTMLInputElement>('input[type="search"]');
		if (!search) return;
		event.preventDefault();
		search.focus();
		search.select();
	}

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

<svelte:window onkeydown={keydown} onclick={closeMenuOutside} />

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<header class="border-b border-stone-200 bg-white">
	<div
		class="mx-auto flex max-w-6xl items-center justify-between gap-3 px-4 py-3 sm:flex-wrap sm:gap-x-4 sm:gap-y-2 sm:px-8"
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
			<form
				class="relative min-w-0 flex-1 sm:max-w-xs"
				method="GET"
				action="{profile}/search"
				role="search"
			>
				<input
					class="peer w-full rounded border border-stone-300 px-2 py-1 text-sm sm:pr-7"
					type="search"
					name="q"
					placeholder="Search {page.params.username}"
					aria-label="Search the artists, albums and tracks of this profile"
					aria-keyshortcuts="/"
				/>
				<kbd
					class="pointer-events-none absolute top-1/2 right-2 hidden -translate-y-1/2 rounded border border-stone-300 px-1 font-mono text-xs leading-4 text-stone-400 peer-focus:hidden peer-[:not(:placeholder-shown)]:hidden sm:block"
					title="Press / to search">/</kbd
				>
				{#if page.url.searchParams.has('source')}
					<input type="hidden" name="source" value={page.url.searchParams.get('source')} />
				{/if}
			</form>
		{/if}
		<nav class="relative shrink-0 text-sm" bind:this={menu}>
			{#if data.me}
				<button
					type="button"
					class="flex items-center rounded px-1.5 py-1 text-stone-600 hover:bg-stone-100 sm:hidden"
					aria-expanded={menuOpen}
					aria-controls="account-menu"
					aria-label="Menu"
					onclick={() => (menuOpen = !menuOpen)}
				>
					<svg class="size-5" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
						<path
							d="M3 5h14a1 1 0 1 0 0-2H3a1 1 0 0 0 0 2Zm14 4H3a1 1 0 0 0 0 2h14a1 1 0 1 0 0-2Zm0 6H3a1 1 0 1 0 0 2h14a1 1 0 1 0 0-2Z"
						/>
					</svg>
				</button>
				<div
					id="account-menu"
					class="{menuOpen
						? 'flex'
						: 'hidden'} absolute top-full right-0 z-20 mt-2 w-44 flex-col rounded border border-stone-200 bg-white py-1 shadow-lg *:px-3 *:py-2 *:text-left sm:static sm:mt-0 sm:flex sm:w-auto sm:flex-row sm:items-baseline sm:gap-4 sm:border-0 sm:bg-transparent sm:py-0 sm:shadow-none sm:*:p-0"
				>
					<a class="font-medium hover:underline" href="/u/{encodeURIComponent(data.me.username)}"
						>{data.me.username}</a
					>
					{#if data.me.admin}
						<a class="text-stone-600 hover:underline" href="/status">Status</a>
					{/if}
					<a class="text-stone-600 hover:underline" href="/settings">Settings</a>
					<button class="cursor-pointer text-stone-600 hover:underline" onclick={logOut}
						>Log out</button
					>
				</div>
			{:else}
				<a class="text-stone-600 hover:underline" href={loginHref}>Log in</a>
			{/if}
		</nav>
	</div>
</header>
<BananaEgg bind:open={bananaOpen} />

{@render children()}
