<script lang="ts">
	import { entityPath, type ChartEntry, type CompareSide, type SharedEntry } from '#lib/api.ts';
	import Chart from '#lib/components/Chart.svelte';
	import PeriodPicker from '#lib/components/PeriodPicker.svelte';
	import SharedChart from '#lib/components/SharedChart.svelte';
	import { combinedYears, comparePath, earliest, latest, scoreLabel } from '#lib/compare.ts';
	import { formatPercent, listenCount } from '#lib/format.ts';
	import { m } from '#lib/i18n/index.ts';
	import { periodLabel, periodSearch } from '#lib/period.ts';
	import { withSource } from '#lib/source.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const c = $derived(data.comparison);
	const [overviewA, overviewB] = $derived(data.overviews);
	const [baseA, baseB] = $derived(data.bases);
	const label = (side: CompareSide) =>
		side.slug === 'default' ? side.username : `${side.username} / ${side.name}`;
	const names = $derived([label(c.a), label(c.b)] as [string, string]);

	const years = $derived(combinedYears(overviewA.years, overviewB.years));
	const search = $derived(withSource(periodSearch(data.period), data.source));
	const swapped = $derived(comparePath(baseB, c.a.username, c.a.slug) + search);

	// Shared entries and the "only" artists link to the pages of the first profile
	// that heard them, as both profiles have pages for them.
	const shared = (base: string, kind: 'artist' | 'album' | 'track') => (entry: SharedEntry) =>
		entityPath(base, kind, entry.id, entry.name) + withSource('', data.source);
	const artistOf = (base: string) => (entry: ChartEntry) =>
		entityPath(base, 'artist', entry.id, entry.name) + withSource('', data.source);
</script>

<svelte:head>
	<title>{m.compare.title(names[0], names[1])} · musicbanana</title>
</svelte:head>

{#snippet side(info: CompareSide, base: string, color: string)}
	<div class="min-w-0">
		<a
			class="flex min-w-0 items-center gap-2 text-xl font-bold hover:underline"
			href={base + search}
		>
			<span class="size-3 shrink-0 rounded-full" style:background-color="var({color})"></span>
			<span class="truncate">{label(info)}</span>
		</a>
		<p class="mt-0.5 text-sm text-stone-600">
			{listenCount(info.listens)} · {m.compare.artists(info.artists)}
		</p>
	</div>
{/snippet}

<main class="mx-auto max-w-6xl px-4 pb-16 sm:px-8">
	<p class="mt-6 text-sm text-stone-500">
		<a class="hover:text-stone-900 hover:underline" href={baseA}>{names[0]}</a>
		· <a class="hover:text-stone-900 hover:underline" href="{baseA}/compare">{m.compare.crumb}</a>
	</p>

	<header class="mt-3 grid items-start gap-4 sm:grid-cols-[1fr_auto_1fr]">
		{@render side(c.a, baseA, '--series-1')}
		<a
			class="self-center justify-self-start rounded px-2 py-1 text-sm text-stone-600 hover:bg-stone-200 sm:justify-self-center"
			href={swapped}
			title={m.compare.swapTitle}>{m.compare.swap}</a
		>
		{@render side(c.b, baseB, '--series-3')}
	</header>

	<section
		class="mt-6 rounded-lg bg-yellow-100/70 px-4 py-4 sm:px-6"
		aria-label={m.compare.overlap}
	>
		<p class="flex flex-wrap items-baseline gap-x-3">
			<span class="text-5xl font-bold tabular-nums">{formatPercent(c.score)}</span>
			<span class="text-xl font-semibold">{scoreLabel(c.score)}</span>
		</p>
		<p class="mt-1 text-sm text-stone-600">
			{m.compare.scoreNote(periodLabel(data.period))}
		</p>
		<p class="mt-2 text-sm text-stone-700">
			{m.compare.inCommon(c.shared.artists, c.shared.releases, c.shared.recordings)}
		</p>
	</section>

	<PeriodPicker
		period={data.period}
		{years}
		first={earliest(overviewA.first_listened_at, overviewB.first_listened_at)}
		last={latest(overviewA.last_listened_at, overviewB.last_listened_at)}
	/>

	<h2 class="mt-10 text-lg font-semibold">{m.compare.both}</h2>
	<div class="mt-3 grid gap-8 md:grid-cols-3">
		<SharedChart
			title={m.common.artists}
			entries={c.artists}
			{names}
			href={shared(baseA, 'artist')}
		/>
		<SharedChart
			title={m.common.albums}
			entries={c.releases}
			{names}
			href={shared(baseA, 'album')}
		/>
		<SharedChart
			title={m.common.tracks}
			entries={c.recordings}
			{names}
			href={shared(baseA, 'track')}
		/>
	</div>

	<h2 class="mt-12 text-lg font-semibold">{m.compare.onlyOne}</h2>
	<p class="mt-1 text-sm text-stone-600">{m.compare.onlyIntro}</p>
	<div class="mt-3 grid gap-8 md:grid-cols-2">
		<div>
			<Chart title={m.compare.only(names[0])} entries={c.a.only} href={artistOf(baseA)} />
			{#if c.a.only_count > c.a.only.length}
				<p class="mt-1 px-2 text-sm text-stone-500">
					{m.compare.allArtists(c.a.only_count)}
				</p>
			{/if}
		</div>
		<div>
			<Chart title={m.compare.only(names[1])} entries={c.b.only} href={artistOf(baseB)} />
			{#if c.b.only_count > c.b.only.length}
				<p class="mt-1 px-2 text-sm text-stone-500">
					{m.compare.allArtists(c.b.only_count)}
				</p>
			{/if}
		</div>
	</div>
</main>
