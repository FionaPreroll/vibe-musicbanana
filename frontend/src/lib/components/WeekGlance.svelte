<script lang="ts">
	import type { Week } from '#lib/api.ts';
	import { formatCalendarDay, formatNumber, listenCount } from '#lib/format.ts';
	import { parseDay } from '#lib/period.ts';

	let {
		week,
		artistHref
	}: {
		week: Week;
		/** The page of an artist of the week. */
		artistHref: (id: number, name: string) => string;
	} = $props();

	const weekday = new Intl.DateTimeFormat(undefined, { weekday: 'short' });
	const shortDay = new Intl.DateTimeFormat(undefined, {
		weekday: 'short',
		day: 'numeric',
		month: 'short'
	});
	const formatDay = (day: string) => shortDay.format(parseDay(day));
	// Streaks can reach back years.
	const formatLongAgo = (day: string) => formatCalendarDay(parseDay(day));
	const days = (n: number) => `${formatNumber(n)} ${n === 1 ? 'day' : 'days'}`;

	// Days of this week so far, today included.
	const daysSoFar = $derived(week.days.filter((d) => d.date <= week.day).length);
	const daysHeard = $derived(week.days.filter((d) => d.date <= week.day && d.listens > 0).length);

	const comparison = $derived.by(() => {
		const diff = week.listens - week.last_week_so_far;
		const by = week.day === week.to ? '' : ' by now';
		if (week.last_week_so_far === 0) return `none last week${by}`;
		if (diff === 0) return `as many as last week${by}`;
		return `${formatNumber(Math.abs(diff))} ${diff > 0 ? 'more' : 'fewer'} than last week${by}`;
	});

	const max = $derived(Math.max(1, ...week.days.flatMap((d) => [d.listens, d.last_week])));
	const height = (n: number) => (n === 0 ? 0 : Math.max(3, (n / max) * 100));
	const lastWeekDay = (day: string) => {
		const date = parseDay(day);
		date.setDate(date.getDate() - 7);
		return shortDay.format(date);
	};

	const artistMax = $derived(week.artists[0]?.listens ?? 1);

	const label = 'text-xs font-medium text-stone-500';
</script>

<section class="mt-8">
	<h2 class="text-sm font-semibold tracking-wide text-stone-500 uppercase">This week</h2>
	<p class="text-sm text-stone-500">{formatDay(week.from)} to {formatDay(week.to)}</p>

	<dl class="mt-3 grid grid-cols-2 gap-x-6 gap-y-4 sm:grid-cols-4">
		<div>
			<dt class={label}>Listens</dt>
			<dd class="text-2xl font-semibold tabular-nums">{formatNumber(week.listens)}</dd>
			<dd class="text-sm text-stone-500">{comparison}</dd>
		</div>
		<div>
			<dt class={label}>Days with listens</dt>
			<dd class="text-2xl font-semibold tabular-nums">
				{daysHeard}<span class="text-base font-normal text-stone-500">{' '}of {daysSoFar}</span>
			</dd>
		</div>
		{#if week.streak}
			{@const streak = week.streak}
			<div>
				<dt class={label}>Streak</dt>
				<dd class="text-2xl font-semibold tabular-nums">{days(streak.current)}</dd>
				<dd class="text-sm text-stone-500">
					{#if streak.current_from}
						in a row since {formatLongAgo(streak.current_from)}
					{:else}
						no listens yesterday or today
					{/if}
				</dd>
			</div>
			<div>
				<dt class={label}>Longest streak</dt>
				<dd class="text-2xl font-semibold tabular-nums">{days(streak.longest)}</dd>
				{#if streak.longest_from && streak.longest_to}
					<dd class="text-sm text-stone-500">
						{#if streak.longest === 1}
							on {formatLongAgo(streak.longest_from)}
						{:else}
							from {formatLongAgo(streak.longest_from)} to {formatLongAgo(streak.longest_to)}
						{/if}
					</dd>
				{/if}
			</div>
		{/if}
	</dl>

	<div class="mt-6 grid gap-8 md:grid-cols-2">
		<figure>
			<figcaption class="mb-2 flex flex-wrap items-center gap-x-4 text-xs text-stone-500">
				<span class="flex items-center gap-1.5">
					<span class="size-2.5 rounded-sm bg-yellow-500"></span>This week
				</span>
				<span class="flex items-center gap-1.5">
					<span class="size-2.5 rounded-sm bg-stone-300"></span>Last week
				</span>
			</figcaption>
			<ol class="grid grid-cols-7 gap-1">
				{#each week.days as d (d.date)}
					{@const later = d.date > week.day}
					<li
						class="flex flex-col"
						title="{formatDay(d.date)}: {later ? 'still to come' : listenCount(d.listens)}
{lastWeekDay(d.date)}: {listenCount(d.last_week)}"
					>
						<div class="flex h-24 items-end justify-center gap-0.5 border-b border-stone-300">
							<div
								class="w-2/5 max-w-4 rounded-t bg-stone-300"
								style:height="{height(d.last_week)}%"
							></div>
							<div
								class="w-2/5 max-w-4 rounded-t bg-yellow-500"
								style:height="{later ? 0 : height(d.listens)}%"
							></div>
						</div>
						<span
							class="mt-1 text-center text-xs {d.date === week.day
								? 'font-semibold text-stone-800'
								: later
									? 'text-stone-400'
									: 'text-stone-500'}">{weekday.format(parseDay(d.date))}</span
						>
					</li>
				{/each}
			</ol>
		</figure>

		<div>
			<h3 class={label}>
				Artists of the week{#if week.new_artists > 0}
					<span class="font-normal">
						· {formatNumber(week.new_artists)} heard for the first time</span
					>{/if}
			</h3>
			{#if week.artists.length === 0}
				<p class="mt-2 text-sm text-stone-500">No listens yet this week.</p>
			{:else}
				<ol class="mt-2 space-y-0.5">
					{#each week.artists as artist (artist.id)}
						<li class="relative overflow-hidden rounded">
							<div
								class="absolute inset-y-0 left-0 bg-yellow-200/70"
								style:width="{(artist.listens / artistMax) * 100}%"
							></div>
							<a
								href={artistHref(artist.id, artist.name)}
								class="relative flex items-baseline gap-2 px-2 py-1 hover:bg-stone-900/5"
							>
								<span class="min-w-0 truncate font-medium">{artist.name}</span>
								{#if artist.new}
									<span
										class="shrink-0 rounded border border-stone-400 px-1 text-xs leading-4 text-stone-600"
										>new</span
									>
								{/if}
								<span class="ml-auto shrink-0 text-sm text-stone-600 tabular-nums">
									{formatNumber(artist.listens)}
									{#if !artist.new}
										<span class="text-stone-400">· {formatNumber(artist.last_week)} last week</span>
									{/if}
								</span>
							</a>
						</li>
					{/each}
				</ol>
			{/if}
		</div>
	</div>
</section>
