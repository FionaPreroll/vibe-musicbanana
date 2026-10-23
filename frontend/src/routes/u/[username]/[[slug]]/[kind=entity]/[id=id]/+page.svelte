<script lang="ts">
	import { entityPath, type ChartEntry, type EntityKind } from '#lib/api.ts';
	import Chart from '#lib/components/Chart.svelte';
	import SourcePicker from '#lib/components/SourcePicker.svelte';
	import MonthCurve from '#lib/components/MonthCurve.svelte';
	import {
		formatDate,
		formatMonth,
		formatNumber,
		formatPercent,
		listenCount
	} from '#lib/format.ts';
	import { withSource } from '#lib/source.ts';
	import { coverUrl, coversShown } from '#lib/covers.svelte.ts';
	import Cover from '#lib/components/Cover.svelte';
	import Fill from '#lib/i18n/Fill.svelte';
	import { m } from '#lib/i18n/index.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const entity = $derived(data.entity);
	const profile = $derived(entity.profile);
	// Where MusicBrainz has the artist, the album's release group or the track's recording.
	const musicBrainzKinds: Record<EntityKind, string> = {
		artist: 'artist',
		album: 'release-group',
		track: 'recording'
	};
	const musicBrainz = (mbid: string) =>
		`https://musicbrainz.org/${musicBrainzKinds[data.kind]}/${mbid}`;

	// Links stay with the chosen source.
	const keep = $derived(withSource('', data.source));
	const albumHref = (entry: ChartEntry) =>
		entityPath(data.base, 'album', entry.id, entry.name) + keep;
	const trackHref = (entry: ChartEntry) =>
		entityPath(data.base, 'track', entry.id, entry.name) + keep;
	const albumCover = $derived(
		coversShown() ? (entry: ChartEntry) => coverUrl(data.api, entry.id) : undefined
	);
</script>

<svelte:head>
	<title>{entity.name} · {profile.username} · musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-6xl px-4 pb-16 sm:px-8">
	<header class="mt-6">
		<div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:gap-6">
			{#if data.kind === 'album' && coversShown()}
				<Cover
					src={coverUrl(data.api, entity.id)}
					srcset="{coverUrl(data.api, entity.id)} 1x, {coverUrl(data.api, entity.id, 500)} 2x"
					name={entity.name}
					alt={m.entity.coverOf(entity.name)}
					class="size-40 text-6xl shadow-sm sm:size-48"
				/>
			{/if}
			<div class="min-w-0">
				<p class="text-sm text-stone-500">
					<a class="hover:text-stone-900 hover:underline" href={data.base + keep}
						>{profile.username}{#if profile.slug !== 'default'}&nbsp;/ {profile.name}{/if}</a
					>
					· {m.entity.kinds[data.kind]}
					{#if entity.mbids.length === 1}
						· <a class="hover:text-stone-900 hover:underline" href={musicBrainz(entity.mbids[0])}
							>MusicBrainz</a
						>
					{:else if entity.mbids.length > 1}
						· MusicBrainz:
						{#each entity.mbids as mbid, i (mbid)}
							<a
								class="hover:text-stone-900 hover:underline"
								href={musicBrainz(mbid)}
								aria-label={m.entity.musicBrainzOf(i + 1, entity.mbids.length)}>{i + 1}</a
							>{i < entity.mbids.length - 1 ? ', ' : ''}
						{/each}
					{/if}
				</p>
				<h1 class="mt-1 text-3xl font-bold">{entity.name}</h1>
				{#if entity.artist}
					<p class="mt-1 text-lg text-stone-600">
						<Fill text={m.entity.by}>
							{#snippet artist()}<a
									class="font-medium text-stone-900 hover:underline"
									href={entityPath(data.base, 'artist', entity.artist!.id, entity.artist!.name) +
										keep}>{entity.artist!.name}</a
								>{/snippet}
						</Fill>
					</p>
				{/if}
			</div>
		</div>
		<SourcePicker sources={data.sources} />
		<dl class="mt-5 flex flex-wrap gap-x-10 gap-y-3">
			<div>
				<dt class="text-xs font-semibold tracking-wide text-stone-500 uppercase">
					{m.entity.listens}
				</dt>
				<dd class="text-xl tabular-nums">{formatNumber(entity.listens)}</dd>
			</div>
			{#if entity.first_listened_at && entity.last_listened_at}
				<div>
					<dt class="text-xs font-semibold tracking-wide text-stone-500 uppercase">
						{m.entity.firstListen}
					</dt>
					<dd class="text-xl">
						<time datetime={entity.first_listened_at}>{formatDate(entity.first_listened_at)}</time>
					</dd>
				</div>
				<div>
					<dt class="text-xs font-semibold tracking-wide text-stone-500 uppercase">
						{m.entity.lastListen}
					</dt>
					<dd class="text-xl">
						<time datetime={entity.last_listened_at}>{formatDate(entity.last_listened_at)}</time>
					</dd>
				</div>
			{/if}
		</dl>
	</header>

	<section class="mt-10">
		<h2 class="mb-2 text-sm font-semibold tracking-wide text-stone-500 uppercase">
			{m.entity.perMonth}
		</h2>
		{#if entity.listens === 0}
			<p class="text-sm text-stone-500">{m.entity.notHeard}</p>
		{:else}
			<MonthCurve months={entity.months} phases={entity.phases} />
			{#if entity.phases.length > 0}
				<h3 class="mt-4 text-sm font-medium">{m.entity.heavy}</h3>
				<ul class="mt-1 space-y-0.5">
					{#each entity.phases as phase (phase.from)}
						<li class="flex items-baseline gap-2">
							<span class="size-2.5 shrink-0 rounded-sm bg-yellow-500" aria-hidden="true"></span>
							<span>
								{phase.from === phase.to
									? formatMonth(phase.from)
									: `${formatMonth(phase.from)} – ${formatMonth(phase.to)}`}
								<span class="text-stone-500">
									· {listenCount(phase.listens)} ({formatPercent(phase.listens / entity.listens)})
								</span>
							</span>
						</li>
					{/each}
				</ul>
			{/if}
		{/if}
	</section>

	<div class="mt-10 grid gap-8 md:grid-cols-2">
		{#if data.kind === 'artist'}
			<Chart
				title={m.common.albums}
				entries={entity.releases}
				href={albumHref}
				hideArtist={entity.name}
				cover={albumCover}
			/>
			<Chart
				title={m.common.tracks}
				entries={entity.recordings}
				href={trackHref}
				hideArtist={entity.name}
			/>
		{:else if data.kind === 'album'}
			<Chart
				title={m.common.tracks}
				entries={entity.recordings}
				href={trackHref}
				hideArtist={entity.artist?.name}
			/>
		{:else}
			<Chart
				title={m.entity.onAlbums}
				entries={entity.releases}
				href={albumHref}
				hideArtist={entity.artist?.name}
				cover={albumCover}
			/>
		{/if}
	</div>
</main>
