<script lang="ts">
	import { entityPath, type ChartEntry, type EntityKind } from '#lib/api.ts';
	import Chart from '#lib/components/Chart.svelte';
	import MonthCurve from '#lib/components/MonthCurve.svelte';
	import {
		formatDate,
		formatMonth,
		formatNumber,
		formatPercent,
		listenCount
	} from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const entity = $derived(data.entity);
	const profile = $derived(entity.profile);
	const kindNames: Record<EntityKind, string> = {
		artist: 'Artist',
		album: 'Album',
		track: 'Track'
	};

	// Where MusicBrainz has the artist, the album's release group or the track's recording.
	const musicBrainzKinds: Record<EntityKind, string> = {
		artist: 'artist',
		album: 'release-group',
		track: 'recording'
	};
	const musicBrainz = (mbid: string) =>
		`https://musicbrainz.org/${musicBrainzKinds[data.kind]}/${mbid}`;

	const albumHref = (entry: ChartEntry) => entityPath(data.base, 'album', entry.id, entry.name);
	const trackHref = (entry: ChartEntry) => entityPath(data.base, 'track', entry.id, entry.name);
</script>

<svelte:head>
	<title>{entity.name} · {profile.username} · musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-6xl px-4 pb-16 sm:px-8">
	<header class="mt-6">
		<p class="text-sm text-stone-500">
			<a class="hover:text-stone-900 hover:underline" href={data.base}
				>{profile.username}{#if profile.slug !== 'default'}&nbsp;/ {profile.name}{/if}</a
			>
			· {kindNames[data.kind]}
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
						aria-label="MusicBrainz {i + 1} of {entity.mbids.length}">{i + 1}</a
					>{i < entity.mbids.length - 1 ? ', ' : ''}
				{/each}
			{/if}
		</p>
		<h1 class="mt-1 text-3xl font-bold">{entity.name}</h1>
		{#if entity.artist}
			<p class="mt-1 text-lg text-stone-600">
				by <a
					class="font-medium text-stone-900 hover:underline"
					href={entityPath(data.base, 'artist', entity.artist.id, entity.artist.name)}
					>{entity.artist.name}</a
				>
			</p>
		{/if}
		<dl class="mt-5 flex flex-wrap gap-x-10 gap-y-3">
			<div>
				<dt class="text-xs font-semibold tracking-wide text-stone-500 uppercase">Listens</dt>
				<dd class="text-xl tabular-nums">{formatNumber(entity.listens)}</dd>
			</div>
			{#if entity.first_listened_at && entity.last_listened_at}
				<div>
					<dt class="text-xs font-semibold tracking-wide text-stone-500 uppercase">First listen</dt>
					<dd class="text-xl">
						<time datetime={entity.first_listened_at}>{formatDate(entity.first_listened_at)}</time>
					</dd>
				</div>
				<div>
					<dt class="text-xs font-semibold tracking-wide text-stone-500 uppercase">Last listen</dt>
					<dd class="text-xl">
						<time datetime={entity.last_listened_at}>{formatDate(entity.last_listened_at)}</time>
					</dd>
				</div>
			{/if}
		</dl>
	</header>

	<section class="mt-10">
		<h2 class="mb-2 text-sm font-semibold tracking-wide text-stone-500 uppercase">
			Listens per month
		</h2>
		{#if entity.listens === 0}
			<p class="text-sm text-stone-500">Not heard in this profile.</p>
		{:else}
			<MonthCurve months={entity.months} phases={entity.phases} />
			{#if entity.phases.length > 0}
				<h3 class="mt-4 text-sm font-medium">Heavy listening</h3>
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
			<Chart title="Albums" entries={entity.releases} href={albumHref} hideArtist={entity.name} />
			<Chart title="Tracks" entries={entity.recordings} href={trackHref} hideArtist={entity.name} />
		{:else if data.kind === 'album'}
			<Chart
				title="Tracks"
				entries={entity.recordings}
				href={trackHref}
				hideArtist={entity.artist?.name}
			/>
		{:else}
			<Chart
				title="On albums"
				entries={entity.releases}
				href={albumHref}
				hideArtist={entity.artist?.name}
			/>
		{/if}
	</div>
</main>
