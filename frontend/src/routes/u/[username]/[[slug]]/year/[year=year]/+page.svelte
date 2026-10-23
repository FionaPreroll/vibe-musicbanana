<script lang="ts">
	import { entityPath, type ChartEntry, type EntityKind } from '#lib/api.ts';
	import Chart from '#lib/components/Chart.svelte';
	import SourcePicker from '#lib/components/SourcePicker.svelte';
	import {
		dates,
		formatDateTimeRange,
		formatDuration,
		formatNumber,
		formatPercent,
		listenCount,
		listParts
	} from '#lib/format.ts';
	import Fill from '#lib/i18n/Fill.svelte';
	import { m } from '#lib/i18n/index.ts';
	import { localDay, parseDay } from '#lib/period.ts';
	import { withSource } from '#lib/source.ts';
	import { wallClock, wallDay } from '#lib/zone.svelte.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const review = $derived(data.review);
	const overview = $derived(data.overview);
	const year = $derived(review.year);

	// Links stay with the chosen source.
	const keep = $derived(withSource('', data.source));
	const yearHref = (y: number) => `${data.base}/year/${y}${keep}`;
	const entryHref = (kind: EntityKind, id: number, name: string) =>
		entityPath(data.base, kind, id, name) + keep;
	const link = (kind: EntityKind) => (entry: ChartEntry) => entryHref(kind, entry.id, entry.name);

	// The years before and after with listens.
	const earlier = $derived(overview.years.findLast((y) => y.year < year)?.year ?? null);
	const later = $derived(overview.years.find((y) => y.year > year)?.year ?? null);

	// A running year compares with the year before up to the same day.
	const partial = $derived(!review.complete);
	const comparison = $derived.by(() => {
		const before = review.last_year_so_far;
		const change = before === 0 ? 0 : review.listens / before - 1;
		const rounded = Math.abs(change) < 0.005 ? 0 : change;
		return m.review.compare(rounded, before, year - 1, partial, formatPercent(Math.abs(change)));
	});

	// The days of the year, or of it so far.
	const daysInYear = $derived.by(() => {
		const start = localDay(year, 1, 1);
		const end = review.complete ? localDay(year + 1, 1, 1) : wallClock(new Date());
		return Math.max(1, Math.ceil((end.getTime() - start.getTime()) / 86_400_000));
	});

	// Days within the year go without it. A string is an instant from the API, a
	// Date a calendar day.
	const dayMonth = dates({ day: 'numeric', month: 'short' });
	const dayOfYear = (when: string | Date) =>
		dayMonth().format(typeof when === 'string' ? wallDay(when) : when);

	const monthName = dates({ month: 'long' });
	const monthShort = dates({ month: 'narrow' });
	const month = (n: number, format = monthName) => format().format(localDay(year, n, 1));
	const topMonth = $derived(
		review.months.reduce((best, m) => (m.listens > best.listens ? m : best), review.months[0])
	);
	const monthMax = $derived(Math.max(1, ...review.months.flatMap((m) => [m.listens, m.last_year])));
	const height = (n: number) => (n === 0 ? 0 : Math.max(2, (n / monthMax) * 100));

	const session = $derived(review.longest_session);
	const sessionLength = $derived(
		session ? Date.parse(session.ended_at) - Date.parse(session.started_at) : 0
	);

	const discoveryMax = $derived(review.discoveries[0]?.listens ?? 1);
	const riserMax = $derived(Math.max(1, ...review.risers.map((r) => r.listens)));

	const label = 'text-xs font-medium text-stone-500';
	const heading = 'mb-2 text-sm font-semibold tracking-wide text-stone-500 uppercase';
</script>

<svelte:head>
	<title>{m.review.title(year)} · {overview.username} · musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-6xl px-4 pb-16 sm:px-8">
	<header class="mt-6">
		<p class="text-sm text-stone-500">
			<a
				class="hover:text-stone-900 hover:underline"
				href={data.base + withSource(`?year=${year}`, data.source)}
				>{overview.username}{#if overview.slug !== 'default'}&nbsp;/ {overview.name}{/if}</a
			>
			· {m.review.crumb}
		</p>
		<div class="mt-1 flex flex-wrap items-baseline justify-between gap-x-6 gap-y-2">
			<h1 class="text-3xl font-bold">
				{m.review.title(year)}{#if !review.complete}{' '}<span class="font-normal text-stone-500"
						>{m.review.soFar}</span
					>{/if}
			</h1>
			<nav class="flex gap-1 text-sm" aria-label={m.review.otherYears}>
				{#if earlier !== null}
					<a class="rounded px-2 py-1 text-stone-600 hover:bg-stone-200" href={yearHref(earlier)}
						>← {earlier}</a
					>
				{/if}
				{#if later !== null}
					<a class="rounded px-2 py-1 text-stone-600 hover:bg-stone-200" href={yearHref(later)}
						>{later} →</a
					>
				{/if}
			</nav>
		</div>
	</header>

	<SourcePicker sources={data.sources} />

	{#if review.listens === 0}
		<p class="mt-8 text-stone-500">{m.review.noListens(year, partial)}</p>
	{:else}
		<dl class="mt-8 grid grid-cols-2 gap-x-6 gap-y-5 sm:grid-cols-3 lg:grid-cols-6">
			<div>
				<dt class={label}>{m.entity.listens}</dt>
				<dd class="text-2xl font-semibold tabular-nums">{formatNumber(review.listens)}</dd>
				<dd class="text-sm text-stone-500">{comparison}</dd>
			</div>
			<div>
				<dt class={label}>{m.common.artists}</dt>
				<dd class="text-2xl font-semibold tabular-nums">{formatNumber(review.artists)}</dd>
				<dd class="text-sm text-stone-500">
					{m.review.newAndTracks(review.new_artists, review.recordings)}
				</dd>
			</div>
			<div>
				<dt class={label}>{m.review.daysWithListens}</dt>
				<dd class="text-2xl font-semibold tabular-nums">
					{formatNumber(review.days)}<span class="text-base font-normal text-stone-500"
						>{' '}{m.week.of(daysInYear)}</span
					>
				</dd>
			</div>
			<div>
				<dt class={label}>{m.review.busiestMonth}</dt>
				<dd class="text-2xl font-semibold">{month(topMonth.month)}</dd>
				<dd class="text-sm text-stone-500">{listenCount(topMonth.listens)}</dd>
			</div>
			{#if review.top_day}
				<div>
					<dt class={label}>{m.review.busiestDay}</dt>
					<dd class="text-2xl font-semibold">{dayOfYear(parseDay(review.top_day.date))}</dd>
					<dd class="text-sm text-stone-500">{listenCount(review.top_day.listens)}</dd>
				</div>
			{/if}
			{#if session}
				<div>
					<dt class={label}>{m.review.longestSession}</dt>
					<dd class="text-2xl font-semibold tabular-nums">
						{sessionLength >= 60_000 ? formatDuration(sessionLength) : listenCount(session.listens)}
					</dd>
					<dd class="text-sm text-stone-500">
						{sessionLength >= 60_000
							? m.review.listensOn(listenCount(session.listens), dayOfYear(session.started_at))
							: dayOfYear(session.started_at)}
					</dd>
				</div>
			{/if}
		</dl>

		<figure class="mt-8">
			<figcaption class="mb-2 flex flex-wrap items-center gap-x-4 text-xs text-stone-500">
				<span class="flex items-center gap-1.5">
					<span class="size-2.5 rounded-sm bg-yellow-500"></span>{year}
				</span>
				<span class="flex items-center gap-1.5">
					<span class="size-2.5 rounded-sm bg-stone-300"></span>{year - 1}
				</span>
			</figcaption>
			<ol class="grid grid-cols-12 gap-1">
				{#each review.months as m (m.month)}
					<li
						class="flex flex-col"
						title="{month(m.month)} {year}: {listenCount(m.listens)}
{month(m.month)} {year - 1}: {listenCount(m.last_year)}"
					>
						<div class="flex h-32 items-end justify-center gap-0.5 border-b border-stone-300">
							<div
								class="w-2/5 max-w-5 rounded-t bg-stone-300"
								style:height="{height(m.last_year)}%"
							></div>
							<div
								class="w-2/5 max-w-5 rounded-t bg-yellow-500"
								style:height="{height(m.listens)}%"
							></div>
						</div>
						<span
							class="mt-1 text-center text-xs {m.month === topMonth.month
								? 'font-semibold text-stone-800'
								: 'text-stone-500'}"
						>
							<span class="sm:hidden">{month(m.month, monthShort)}</span>
							<span class="hidden sm:inline">{month(m.month).slice(0, 3)}</span>
						</span>
					</li>
				{/each}
			</ol>
		</figure>

		<div class="mt-10 grid gap-8 md:grid-cols-3">
			<Chart title={m.common.topArtists} entries={data.artists} href={link('artist')} />
			<Chart title={m.common.topAlbums} entries={data.releases} href={link('album')} />
			<Chart title={m.common.topTracks} entries={data.recordings} href={link('track')} />
		</div>

		<div class="mt-12 grid gap-8 md:grid-cols-2">
			<section>
				<h2 class={heading}>{m.review.discovered(year)}</h2>
				{#if review.discoveries.length === 0}
					<p class="text-sm text-stone-500">{m.review.noDiscoveries}</p>
				{:else}
					<p class="mb-2 text-sm text-stone-500">
						{m.review.discoveries(
							review.new_artists,
							review.new_artists > review.discoveries.length
						)}
					</p>
					<ol class="space-y-0.5">
						{#each review.discoveries as artist (artist.id)}
							<li class="relative overflow-hidden rounded">
								<div
									class="absolute inset-y-0 left-0 bg-yellow-200/70"
									style:width="{(artist.listens / discoveryMax) * 100}%"
								></div>
								<a
									href={entryHref('artist', artist.id, artist.name)}
									class="relative flex items-baseline gap-2 px-2 py-1 hover:bg-stone-900/5"
								>
									<span class="min-w-0 truncate font-medium" title={artist.name}>{artist.name}</span
									>
									<span class="ml-auto shrink-0 text-sm text-stone-600 tabular-nums">
										{formatNumber(artist.listens)}
										<span class="text-stone-400"
											>· {m.review.since(dayOfYear(artist.first_listened_at))}</span
										>
									</span>
								</a>
							</li>
						{/each}
					</ol>
				{/if}
			</section>

			<section>
				<h2 class={heading}>{m.review.risers}</h2>
				{#if review.risers.length === 0}
					<p class="text-sm text-stone-500">
						{m.review.noRisers(year - 1, partial)}
					</p>
				{:else}
					<p class="mb-2 text-sm text-stone-500">
						{m.review.risersSub(year - 1, partial)}
					</p>
					<ol class="space-y-0.5">
						{#each review.risers as artist (artist.id)}
							<li class="relative overflow-hidden rounded">
								<div
									class="absolute inset-y-0 left-0 bg-stone-300"
									style:width="{(artist.last_year / riserMax) * 100}%"
								></div>
								<div
									class="absolute inset-y-0 bg-yellow-200/70"
									style:left="{(artist.last_year / riserMax) * 100}%"
									style:width="{((artist.listens - artist.last_year) / riserMax) * 100}%"
								></div>
								<a
									href={entryHref('artist', artist.id, artist.name)}
									class="relative flex items-baseline gap-2 px-2 py-1 hover:bg-stone-900/5"
								>
									<span class="min-w-0 truncate font-medium" title={artist.name}>{artist.name}</span
									>
									{#if artist.last_rank === null}
										<span
											class="shrink-0 rounded border border-stone-400 px-1 text-xs leading-4 text-stone-600"
											title={m.review.notHeardIn(year - 1, partial)}>{m.review.back}</span
										>
									{:else if artist.last_rank > artist.rank}
										<span
											class="shrink-0 rounded border border-stone-400 px-1 text-xs leading-4 text-stone-600 tabular-nums"
											title={m.review.places(year - 1, year, partial)}
											>#{artist.last_rank} → #{artist.rank}</span
										>
									{/if}
									<span class="ml-auto shrink-0 text-sm text-stone-600 tabular-nums">
										+{formatNumber(artist.listens - artist.last_year)}
										<span class="text-stone-400">· {formatNumber(artist.listens)}</span>
									</span>
								</a>
							</li>
						{/each}
					</ol>
				{/if}
			</section>
		</div>

		{#if session && session.listens > 1}
			<section class="mt-12">
				<h2 class={heading}>{m.review.longestSession}</h2>
				<p>
					<span class="font-medium"
						>{formatDateTimeRange(session.started_at, session.ended_at)}</span
					>
					<span class="text-stone-500">
						· {formatDuration(sessionLength)} · {listenCount(session.listens)}
					</span>
				</p>
				<p class="mt-1 text-sm text-stone-500">
					<Fill text={m.review.mostly}>
						{#snippet artists()}{#each listParts(session.artists.length) as part, i (i)}{#if part.index < 0}{part.text}{:else}{@const artist =
										session.artists[part.index]}<a
										class="text-stone-700 hover:underline"
										href={entryHref('artist', artist.id, artist.name)}>{artist.name}</a
									>&nbsp;({formatNumber(artist.listens)}){/if}{/each}{/snippet}
					</Fill>
				</p>
			</section>
		{/if}
	{/if}
</main>
