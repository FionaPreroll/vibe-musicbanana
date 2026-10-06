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
	import FollowButton from '#lib/components/FollowButton.svelte';
	import ListeningClock from '#lib/components/ListeningClock.svelte';
	import ListenTime from '#lib/components/ListenTime.svelte';
	import LostAndFound from '#lib/components/LostAndFound.svelte';
	import OnThisDay from '#lib/components/OnThisDay.svelte';
	import PeriodPicker from '#lib/components/PeriodPicker.svelte';
	import SourcePicker from '#lib/components/SourcePicker.svelte';
	import VisibilityBadge from '#lib/components/VisibilityBadge.svelte';
	import WeekGlance from '#lib/components/WeekGlance.svelte';
	import { byDay, formatDate, formatDay, listenCount } from '#lib/format.ts';
	import { now } from '#lib/now.svelte.ts';
	import { parseDay, periodEnd, periodSearch, type Period } from '#lib/period.ts';
	import { withSource } from '#lib/source.ts';
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

	// Entry pages stay with the chosen source.
	const entryHref = (kind: EntityKind, id: number, name: string) =>
		entityPath(data.base, kind, id, name) + withSource('', data.source);
	const link = (kind: EntityKind) => (entry: ChartEntry) => entryHref(kind, entry.id, entry.name);

	// A profile nobody listened to lately keeps its page as it was.
	const week = $derived(
		data.week && (data.week.listens || data.week.last_week || data.week.streak.current)
			? data.week
			: null
	);

	// The top artists of every year don't depend on the period, so they load once per
	// profile and after the rest of the page. (`data` is new after every navigation,
	// `api` only for another profile.)
	const api = $derived(data.api);
	const source = $derived(data.source);
	let artistYears: YearTop[] = $state([]);
	$effect(() => {
		let current = true;
		artistYears = [];
		getJson<YearTop[]>(fetch, `${api}/top/artists/years`, { tz: timeZone, limit: 10, source })
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
	const days = $derived(byDay(listens, (l) => l.listened_at));
	let nowPlaying = $derived(data.nowPlaying);

	// While the page is open, the server says when "now playing" or the listens
	// change (backend/src/live.rs), and new listens go on top unless the period is
	// over. A hidden tab lets go of its connection and catches up when it shows again.
	$effect(() => {
		const api = data.api;
		const source = data.source;
		const live = periodEnd(data.period) === null;
		let events: EventSource | null = null;

		function connect(catchUp: boolean) {
			events = new EventSource(`${api}/live`);
			events.addEventListener('now-playing', () => refreshNowPlaying(api));
			events.addEventListener('listens', () => refresh(api, source, live));
			// After a reconnect, events in between may be lost.
			events.onopen = () => {
				if (catchUp) refresh(api, source, live);
				catchUp = true;
			};
		}
		function visibility() {
			if (document.hidden) {
				events?.close();
				events = null;
			} else if (!events) {
				connect(true);
			}
		}

		if (!document.hidden) connect(false);
		document.addEventListener('visibilitychange', visibility);
		return () => {
			document.removeEventListener('visibilitychange', visibility);
			events?.close();
		};
	});

	// "Now playing" ends after the track's length, or ten minutes without one.
	$effect(() => {
		if (!nowPlaying) return;
		const api = data.api;
		const ends = Date.parse(nowPlaying.started_at) + (nowPlaying.duration_ms ?? 600_000);
		const timer = setTimeout(() => refreshNowPlaying(api), Math.max(ends - Date.now(), 0) + 1000);
		return () => clearTimeout(timer);
	});

	async function refreshNowPlaying(api: string) {
		try {
			nowPlaying = await getJson<NowPlaying | null>(fetch, `${api}/now-playing`);
		} catch {
			// Offline or a server restart; the next change tries again.
		}
	}

	async function refresh(api: string, source: string | null, live: boolean) {
		try {
			nowPlaying = await getJson<NowPlaying | null>(fetch, `${api}/now-playing`);
			if (!live) return;
			const latest = await getJson<ListensPage>(fetch, `${api}/listens`, { limit: 25, source });
			const newest = listens.length ? Date.parse(listens[0].listened_at) : -Infinity;
			const fresh = latest.listens.filter((l) => Date.parse(l.listened_at) > newest);
			if (fresh.length > 0) listens = [...fresh, ...listens];
		} catch {
			// Offline or a server restart; the next change tries again.
		}
	}

	async function loadMore() {
		if (!next || loadingMore) return;
		loadingMore = true;
		try {
			const older = await getJson<ListensPage>(fetch, `${data.api}/listens`, {
				before: next,
				limit: 50,
				source: data.source
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
		{#if data.follow}
			<div class="mt-2"><FollowButton info={data.follow} /></div>
		{/if}
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

	{#if week}
		<WeekGlance {week} artistHref={(id, name) => entryHref('artist', id, name)} />
	{/if}

	{#if data.onThisDay && data.onThisDay.years.length > 0}
		<OnThisDay
			onThisDay={data.onThisDay}
			href={entryHref}
			dayHref={(day) =>
				data.base + withSource(periodSearch({ kind: 'range', from: day, to: day }), data.source)}
		/>
	{/if}

	<PeriodPicker
		period={data.period}
		{years}
		first={overview.first_listened_at}
		last={overview.last_listened_at}
	/>
	<SourcePicker sources={data.sources} />

	{#if data.period.kind === 'year'}
		<p class="mt-4">
			<a
				class="inline-flex items-baseline gap-1.5 rounded bg-yellow-200/70 px-2.5 py-1 text-sm font-medium hover:bg-yellow-300/70"
				href="{data.base}/year/{data.period.year}{withSource('', data.source)}"
				>{data.period.year} in review →</a
			>
		</p>
	{/if}

	<div class="mt-8 grid gap-8 md:grid-cols-3">
		<Chart title="Top artists" entries={data.artists} href={link('artist')} />
		<Chart title="Top albums" entries={data.releases} href={link('album')} />
		<Chart title="Top tracks" entries={data.recordings} href={link('track')} />
	</div>

	{#if data.clock && data.clock.listens > 0}
		<div class="mt-12">
			<ListeningClock clock={data.clock} />
		</div>
	{/if}

	{#if data.lost && (data.lost.artists.length > 0 || data.lost.tracks.length > 0 || data.lost.hidden)}
		<LostAndFound
			lost={data.lost}
			api={data.api}
			own={overview.own ? overview.slug : null}
			source={data.source}
			href={entryHref}
		/>
	{/if}

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
			{#each days as day (day.key)}
				<h3 class="mt-4 border-b border-stone-200 pb-1 text-sm font-semibold text-stone-700">
					<time datetime={day.key}>{formatDay(day.date, now())}</time>
				</h3>
				<ol class="divide-y divide-stone-200">
					{#each day.items as listen (listen.listened_at)}
						<li class="flex items-baseline gap-3 py-1.5 sm:gap-4">
							<ListenTime
								class="w-28 shrink-0 text-sm text-stone-500 tabular-nums sm:w-32"
								at={listen.listened_at}
							/>
							<span class="min-w-0 truncate">
								<a
									class="font-medium hover:underline"
									href={entryHref('track', listen.recording_id, listen.track)}>{listen.track}</a
								>
								<span class="text-stone-500">
									· <a
										class="hover:underline"
										href={entryHref('artist', listen.artist_id, listen.artist)}>{listen.artist}</a
									>
									{#if listen.album && listen.release_id}
										· <a
											class="hover:underline"
											href={entryHref('album', listen.release_id, listen.album)}>{listen.album}</a
										>
									{/if}
								</span>
							</span>
						</li>
					{/each}
				</ol>
			{/each}
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
