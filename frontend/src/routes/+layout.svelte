<script lang="ts">
	import './layout.css';
	import favicon from '#lib/assets/favicon.svg';

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<header class="border-b border-stone-200 bg-white">
	<div class="mx-auto max-w-6xl px-4 py-3 sm:px-8">
		<a href="/" class="text-lg font-bold">musicbanana 🍌</a>
	</div>
</header>

{@render children()}
