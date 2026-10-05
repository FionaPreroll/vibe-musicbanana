<script lang="ts">
	import { goto } from '$app/navigation';
	import { entityPath, type ChartEntry, type EntityKind } from '#lib/api.ts';
	import Chart from '#lib/components/Chart.svelte';
	import SourcePicker from '#lib/components/SourcePicker.svelte';
	import { withSource } from '#lib/source.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const overview = $derived(data.overview);

	// The address follows the field while typing, so a search can be bookmarked and
	// the back button leaves the page rather than every letter.
	let q = $derived(data.q);
	let timer: ReturnType<typeof setTimeout> | undefined;
	function typed() {
		clearTimeout(timer);
		timer = setTimeout(() => {
			const query = q.trim() ? `?q=${encodeURIComponent(q.trim())}` : '';
			goto(`${data.base}/search${withSource(query, data.source)}`, {
				replace: true,
				reset: false
			});
		}, 250);
	}

	const link = (kind: EntityKind) => (entry: ChartEntry) =>
		entityPath(data.base, kind, entry.id, entry.name) + withSource('', data.source);
	const nothing = $derived(
		data.found.artists.length + data.found.releases.length + data.found.recordings.length === 0
	);
</script>

<svelte:head>
	<title>{data.q ? `${data.q} · ` : ''}Search · {overview.username} · musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-6xl px-4 pb-16 sm:px-8">
	<p class="mt-6 text-sm text-stone-500">
		<a class="hover:text-stone-900 hover:underline" href={data.base + withSource('', data.source)}
			>{overview.username}{#if overview.slug !== 'default'}&nbsp;/ {overview.name}{/if}</a
		>
		· Search
	</p>
	<form
		class="mt-2"
		role="search"
		onsubmit={(e) => {
			e.preventDefault();
			typed();
		}}
	>
		<!-- svelte-ignore a11y_autofocus -->
		<input
			class="w-full rounded border border-stone-300 px-3 py-2 text-lg"
			type="search"
			placeholder="Artist, album or track"
			aria-label="Search the artists, albums and tracks of {overview.username}"
			autofocus
			bind:value={q}
			oninput={typed}
		/>
	</form>
	<SourcePicker sources={data.sources} />

	{#if data.q && nothing}
		<p class="mt-8 text-stone-600">Nothing heard matches “{data.q}”.</p>
	{:else if data.q}
		<div class="mt-8 grid gap-8 md:grid-cols-3">
			<Chart title="Artists" entries={data.found.artists} href={link('artist')} />
			<Chart title="Albums" entries={data.found.releases} href={link('album')} />
			<Chart title="Tracks" entries={data.found.recordings} href={link('track')} />
		</div>
	{:else}
		<p class="mt-8 text-sm text-stone-500">
			Every word counts, in any case and with or without accents; “ärzte unrockbar” finds the track.
		</p>
	{/if}
</main>
