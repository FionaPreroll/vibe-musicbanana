<script lang="ts">
	import { page } from '$app/state';
	import { m } from '#lib/i18n/index.ts';
</script>

<svelte:head>
	<title>{page.status} · musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-2xl px-4 pb-16 sm:px-8">
	<p class="mt-10 text-6xl font-bold text-stone-300">{page.status}</p>
	<p class="mt-2">{page.error?.message}</p>
	<p class="mt-6"><a class="underline" href="/">{m.errors.backHome}</a></p>
</main>
