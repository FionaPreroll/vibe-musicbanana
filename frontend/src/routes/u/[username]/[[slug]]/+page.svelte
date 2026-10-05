<script lang="ts">
	import {
		entityPath,
		getJson,
		timeZone,
		type ChartEntry,
		type EntityKind,
		type ListensPage,
		type NowPlaying,
		type YearTop
	} from '#lib/api.ts';
	import ArtistYears from '#lib/components/ArtistYears.svelte';
	import Chart from '#lib/components/Chart.svelte';
	import PeriodPicker from '#lib/components/PeriodPicker.svelte';
	import VisibilityBadge from '#lib/components/VisibilityBadge.svelte';
	import { formatDate, formatDateTime, listenCount } from '#lib/format.ts';
	import { parseDay, periodEnd, type Period } from '#lib/period.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const overview = $derived(data.overview);

	// Years without listens stay in the bar as gaps (e.g. between an import and new scrobbles).
	const years = $derived.by(() => {
		const first = overview.years.at(0)?.year;
		const last = overview.years.at(-1)?.year;
		if (first === undefined || last === undefined || last - first > 100) return overview.years;
		const listens = new Map(overview.years.map((y) => [y.year, y.listens]));
		return Array.from({ length: last - first + 1 }, (_, i) => ({
			year: first + i,
			listens: listens.get(first + i) ?? 0
		}));
	});

	const link = (kind: EntityKind) => (entry: ChartEntry) =>
		entityPath(data.base, kind, entry.id, entry.name);

	// The top artists of every year don't depend on the period, so they load once per
	// profile and after the rest of the page. (`data` is new after every navigation,
	// `api` only for another profile.)
	const api = $derived(data.api);
	let artistYears: YearTop[] = $state([]);
	$effect(() => {
		let current = true;
		artistYears = [];
		getJson<YearTop[]>(fetch, `${api}/top/artists/years`, { tz: timeZone, limit: 10 })
			.then((years) => {
				if (current) artistYears = years;
			})
			.catch(() => {
				// The chart stays away; the rest of the page works without it.
			});
		return () => (current = false);
	});

	function listensTitle(period: Period) {
		if (period.kind === 'year') return `Last listens of ${period.year}`;
		if (period.kind === 'range' && period.to) {
			return `Last listens up to ${formatDate(parseDay(period.to))}`;
		}
		return 'Recent listens';
	}

	// Start from the loaded page again whenever the profile or period changes.
	let listens = $derived(data.recent.listens);
	let next = $derived(data.recent.next);
	let loadingMore = $state(false);
	let nowPlaying = $derived(data.nowPlaying);

	// While the page is open, keep "now playing" current and put new listens on top
	// unless the period is over.
	$effect(() => {
		const api = data.api;
		const live = periodEnd(data.period) === null;
		const timer = setInterval(() => {
			if (!document.hidden) refresh(api, live);
		}, 30_000);
		return () => clearInterval(timer);
	});

	async function refresh(api: string, live: boolean) {
		try {
			nowPlaying = await getJson<NowPlaying | null>(fetch, `${api}/now-playing`);
			if (!live) return;
			const latest = await getJson<ListensPage>(fetch, `${api}/listens`, { limit: 25 });
			const newest = listens.length ? Date.parse(listens[0].listened_at) : -Infinity;
			const fresh = latest.listens.filter((l) => Date.parse(l.listened_at) > newest);
			if (fresh.length > 0) listens = [...fresh, ...listens];
		} catch {
			// Offline or a server restart; the next round tries again.
		}
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
			<VisibilityBadge visibility={overview.visibility} />
		</h1>
		<p class="mt-1 text-stone-600">
			{listenCount(overview.listens)}
			{#if overview.first_listened_at && overview.last_listened_at}
				from {formatDate(overview.first_listened_at)} to {formatDate(overview.last_listened_at)}
			{/if}
		</p>
		{#if nowPlaying}
			<p class="mt-3 flex items-center gap-2">
				<span class="relative flex size-2.5 shrink-0" aria-hidden="true">
					<span
						class="absolute inline-flex size-full animate-ping rounded-full bg-yellow-400 opacity-75"
					></span>
					<span class="relative inline-flex size-2.5 rounded-full bg-yellow-500"></span>
				</span>
				<span class="shrink-0 text-sm text-stone-500">Now playing</span>
				<span class="min-w-0 truncate">
					<span class="font-medium">{nowPlaying.track}</span>
					<span class="text-stone-500">
						· {[nowPlaying.artist, nowPlaying.album].filter(Boolean).join(' · ')}
					</span>
				</span>
			</p>
		{/if}
	</header>

	<PeriodPicker
		period={data.period}
		{years}
		first={overview.first_listened_at}
		last={overview.last_listened_at}
	/>

	<div class="mt-8 grid gap-8 md:grid-cols-3">
		<Chart title="Top artists" entries={data.artists} href={link('artist')} />
		<Chart title="Top albums" entries={data.releases} href={link('album')} />
		<Chart title="Top tracks" entries={data.recordings} href={link('track')} />
	</div>

	{#if artistYears.length > 0}
		<div class="mt-12">
			<ArtistYears
				years={artistYears}
				base={data.base}
				selected={data.period.kind === 'year' ? data.period.year : null}
			/>
		</div>
	{/if}

	<section class="mt-12">
		<h2 class="mb-2 text-sm font-semibold tracking-wide text-stone-500 uppercase">
			{listensTitle(data.period)}
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
							<a
								class="font-medium hover:underline"
								href={entityPath(data.base, 'track', listen.recording_id, listen.track)}
								>{listen.track}</a
							>
							<span class="text-stone-500">
								· <a
									class="hover:underline"
									href={entityPath(data.base, 'artist', listen.artist_id, listen.artist)}
									>{listen.artist}</a
								>
								{#if listen.album && listen.release_id}
									· <a
										class="hover:underline"
										href={entityPath(data.base, 'album', listen.release_id, listen.album)}
										>{listen.album}</a
									>
								{/if}
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
