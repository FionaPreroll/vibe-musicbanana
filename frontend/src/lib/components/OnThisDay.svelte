<script lang="ts">
	import type { EntityKind, OnThisDay } from '#lib/api.ts';
	import { formatNumber, listenCount } from '#lib/format.ts';
	import { parseDay } from '#lib/period.ts';

	let {
		onThisDay,
		href,
		dayHref
	}: {
		onThisDay: OnThisDay;
		/** The page of an artist or track. */
		href: (kind: EntityKind, id: number, name: string) => string;
		/** The profile page narrowed to one day. */
		dayHref: (day: string) => string;
	} = $props();

	const dayOfYear = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'long' });
	const fullDay = new Intl.DateTimeFormat(undefined, {
		weekday: 'short',
		day: 'numeric',
		month: 'short',
		year: 'numeric'
	});
	const yearsAgo = (n: number) => (n === 1 ? 'A year ago' : `${formatNumber(n)} years ago`);

	// One row of cards at first, so the charts stay close; years go back to 2007 for some.
	const shown = 3;
	let all = $state(false);
	const years = $derived(all ? onThisDay.years : onThisDay.years.slice(0, shown));
</script>

<section class="mt-8">
	<h2 class="text-sm font-semibold tracking-wide text-stone-500 uppercase">On this day</h2>
	<p class="text-sm text-stone-500">
		{dayOfYear.format(parseDay(onThisDay.day))} in earlier years
	</p>

	<ol class="mt-3 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
		{#each years as year (year.date)}
			<li class="rounded-lg border border-stone-200 p-3">
				<a href={dayHref(year.date)} class="group flex items-baseline gap-2">
					<span class="font-semibold group-hover:underline">{yearsAgo(year.years_ago)}</span>
					<span class="text-sm text-stone-500">{fullDay.format(parseDay(year.date))}</span>
					<span class="ml-auto shrink-0 text-sm text-stone-500 tabular-nums"
						>{listenCount(year.listens)}</span
					>
				</a>
				<p class="mt-1 truncate text-sm text-stone-600">
					{#each year.artists as artist, i (artist.id)}{#if i > 0}{' · '}{/if}<a
							class="hover:underline"
							href={href('artist', artist.id, artist.name)}>{artist.name}</a
						>{/each}
				</p>
				<ol class="mt-2 space-y-0.5 text-sm">
					{#each year.tracks as track (track.id)}
						<li class="flex items-baseline gap-2">
							<span class="min-w-0 truncate">
								<a class="font-medium hover:underline" href={href('track', track.id, track.name)}
									>{track.name}</a
								>
								<span class="text-stone-500">· {track.artist}</span>
							</span>
							{#if track.listens > 1}
								<span class="ml-auto shrink-0 text-stone-500 tabular-nums"
									>{formatNumber(track.listens)}×</span
								>
							{/if}
						</li>
					{/each}
				</ol>
			</li>
		{/each}
	</ol>

	{#if onThisDay.years.length > shown}
		<button
			class="mt-3 rounded border border-stone-300 px-3 py-1.5 text-sm hover:bg-stone-100"
			onclick={() => (all = !all)}
		>
			{all ? 'Fewer years' : `All ${formatNumber(onThisDay.years.length)} years`}
		</button>
	{/if}
</section>
