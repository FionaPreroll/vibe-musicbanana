<script lang="ts">
	import { page } from '$app/state';
	import { getJson, type ListensPage } from '#lib/api.ts';
	import Chart from '#lib/components/Chart.svelte';
	import { formatDate, formatDateTime, listenCount } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const overview = $derived(data.overview);
	const busiestYear = $derived(Math.max(1, ...overview.years.map((y) => y.listens)));

	// Start from the loaded page again whenever the profile or year changes.
	let listens = $derived(data.recent.listens);
	let next = $derived(data.recent.next);
	let loadingMore = $state(false);

	// On narrow screens the years scroll sideways; keep the selected one in view.
	function reveal(node: HTMLElement) {
		node.scrollIntoView({ block: 'nearest', inline: 'nearest' });
	}

	async function loadMore() {
		if (!next || loadingMore) return;
		loadingMore = true;
		try {
			const older = await getJson<ListensPage>(fetch, `${data.api}/listens`, {
				before: next,
				limit: 50
			});
			listens = [...listens, ...older.listens];
			next = older.next;
		} finally {
			loadingMore = false;
		}
	}
</script>

<svelte:head>
	<title>{overview.username} · musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-6xl px-4 pb-16 sm:px-8">
	<header class="mt-6">
		<h1 class="text-3xl font-bold">
			{overview.username}
			{#if overview.slug !== 'default'}<span class="font-normal text-stone-500">
					/ {overview.name}</span
				>{/if}
		</h1>
		<p class="mt-1 text-stone-600">
			{listenCount(overview.listens)}
			{#if overview.first_listened_at && overview.last_listened_at}
				from {formatDate(overview.first_listened_at)} to {formatDate(overview.last_listened_at)}
			{/if}
		</p>
	</header>

	{#if overview.years.length > 0}
		<nav class="mt-8 flex items-end gap-2" aria-label="Year">
			<a
				href={page.url.pathname}
				data-sveltekit-noscroll
				class="mb-5 shrink-0 rounded px-2 py-1 text-sm whitespace-nowrap {data.year === null
					? 'bg-stone-900 text-white'
					: 'text-stone-600 hover:bg-stone-200'}"
				aria-current={data.year === null ? 'page' : undefined}
			>
				All time
			</a>
			<div class="flex min-w-0 items-end gap-1 overflow-x-auto">
				{#each overview.years as y (y.year)}
					{@const selected = data.year === y.year}
					<a
						href="?year={y.year}"
						data-sveltekit-noscroll
						class="group flex w-12 shrink-0 flex-col items-center"
						title={listenCount(y.listens)}
						aria-current={selected ? 'page' : undefined}
						{@attach selected && reveal}
					>
						<span class="flex h-16 w-8 items-end">
							<span
								class="w-full rounded-t {selected
									? 'bg-yellow-500'
									: 'bg-yellow-200 group-hover:bg-yellow-300'}"
								style:height="{Math.max(4, (y.listens / busiestYear) * 100)}%"
							></span>
						</span>
						<span
							class="mt-1 text-xs tabular-nums {selected
								? 'font-semibold text-stone-900'
								: 'text-stone-500'}">{y.year}</span
						>
					</a>
				{/each}
			</div>
		</nav>
	{/if}

	<div class="mt-8 grid gap-8 md:grid-cols-3">
		<Chart title="Top artists" entries={data.artists} />
		<Chart title="Top albums" entries={data.releases} />
		<Chart title="Top tracks" entries={data.recordings} />
	</div>

	<section class="mt-12">
		<h2 class="mb-2 text-sm font-semibold tracking-wide text-stone-500 uppercase">
			{data.year === null ? 'Recent listens' : `Last listens of ${data.year}`}
		</h2>
		{#if listens.length === 0}
			<p class="text-sm text-stone-500">No listens.</p>
		{:else}
			<ol class="divide-y divide-stone-200">
				{#each listens as listen (listen.listened_at)}
					<li class="flex flex-col py-1.5 sm:flex-row sm:items-baseline sm:gap-4">
						<time
							class="shrink-0 text-sm text-stone-500 tabular-nums sm:w-44"
							datetime={listen.listened_at}>{formatDateTime(listen.listened_at)}</time
						>
						<span class="min-w-0 truncate">
							<span class="font-medium">{listen.track}</span>
							<span class="text-stone-500">
								· {[listen.artist, listen.album].filter(Boolean).join(' · ')}
							</span>
						</span>
					</li>
				{/each}
			</ol>
			{#if next}
				<button
					class="mt-4 rounded border border-stone-300 px-3 py-1.5 text-sm hover:bg-stone-100 disabled:opacity-50"
					onclick={loadMore}
					disabled={loadingMore}
				>
					{loadingMore ? 'Loading…' : 'Older listens'}
				</button>
			{/if}
		{/if}
	</section>
</main>
