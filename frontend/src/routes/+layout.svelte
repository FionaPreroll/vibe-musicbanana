<script lang="ts">
	import './layout.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { sendJson } from '#lib/api.ts';
	import favicon from '#lib/assets/favicon.svg';

	let { data, children } = $props();

	async function logOut() {
		await sendJson(fetch, 'DELETE', '/api/session');
		await goto('/', { invalidateAll: true });
	}

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
	<div class="mx-auto flex max-w-6xl items-baseline justify-between gap-4 px-4 py-3 sm:px-8">
		<a href="/" class="text-lg font-bold">musicbanana 🍌</a>
		<nav class="flex items-baseline gap-4 text-sm">
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
