<script lang="ts">
	import type { Clock } from '#lib/api.ts';
	import { formatPercent, listenCount } from '#lib/format.ts';
	import { timeZone, weekStart } from '#lib/zone.svelte.ts';

	let { clock }: { clock: Clock } = $props();

	// Monday 2024-01-01 and the days after it name the weekdays, Monday first like the API.
	const day = (i: number, hour = 0) => new Date(2024, 0, 1 + i, hour);
	const shortWeekday = new Intl.DateTimeFormat(undefined, { weekday: 'short' });
	const longWeekday = new Intl.DateTimeFormat(undefined, { weekday: 'long' });
	const hourFormat = new Intl.DateTimeFormat(undefined, { hour: 'numeric' });
	const weekdays = Array.from({ length: 7 }, (_, i) => ({
		short: shortWeekday.format(day(i)),
		long: longWeekday.format(day(i))
	}));
	/** E.g. "9 – 10 PM" or "21–22 Uhr". */
	const hourRange = (hour: number) => hourFormat.formatRange(day(0, hour), day(0, hour + 1));
	const axis = [0, 6, 12, 18].map((hour) => ({ hour, label: hourFormat.format(day(0, hour)) }));

	const sum = (ns: number[]) => ns.reduce((a, b) => a + b, 0);
	const dayTotals = $derived(clock.weekdays.map(sum));
	const hourTotals = $derived(
		Array.from({ length: 24 }, (_, hour) => sum(clock.weekdays.map((hours) => hours[hour])))
	);
	const busiest = (ns: number[]) => ns.indexOf(Math.max(...ns));
	// The rows start with the viewer's first weekday; `i` stays Monday-based.
	const rows = $derived(Array.from({ length: 7 }, (_, n) => (weekStart() - 1 + n) % 7));
	const busiestDay = $derived(busiest(dayTotals));
	const busiestHour = $derived(busiest(hourTotals));

	// Five shades of yellow; the square root keeps quiet hours from all looking empty.
	// The dark theme turns 100 to 300 dark but keeps 400 and 500, so there the top two
	// go on brightening up to banana.
	const shades = [
		'bg-yellow-100',
		'bg-yellow-200',
		'bg-yellow-300',
		'bg-yellow-400 dark:bg-[oklch(70%_0.15_96)]',
		'bg-yellow-500 dark:bg-banana'
	];
	const max = $derived(Math.max(1, ...clock.weekdays.flat()));
	const shade = (n: number) =>
		n === 0 ? 'bg-stone-200/60' : shades[Math.ceil(Math.sqrt(n / max) * shades.length) - 1];

	const hourMax = $derived(Math.max(1, ...hourTotals));
	const dayMax = $derived(Math.max(1, ...dayTotals));
	const share = (n: number) => formatPercent(n / clock.listens);

	const columns =
		'grid-cols-[auto_repeat(24,minmax(0,1fr))] sm:grid-cols-[auto_repeat(24,minmax(0,1fr))_4rem]';
	const label = 'text-xs font-medium text-stone-500';
</script>

<section class="max-w-3xl">
	<h2 class="text-sm font-semibold tracking-wide text-stone-500 uppercase">Listening clock</h2>
	<p class="text-sm text-stone-500">By weekday and hour, in {timeZone().replaceAll('_', ' ')}</p>

	<dl class="mt-3 grid grid-cols-2 gap-x-6 gap-y-4">
		<div>
			<dt class={label}>Busiest day</dt>
			<dd class="text-2xl font-semibold">{weekdays[busiestDay].long}</dd>
			<dd class="text-sm text-stone-500">{share(dayTotals[busiestDay])} of the listens</dd>
		</div>
		<div>
			<dt class={label}>Busiest hour</dt>
			<dd class="text-2xl font-semibold tabular-nums">{hourRange(busiestHour)}</dd>
			<dd class="text-sm text-stone-500">{share(hourTotals[busiestHour])} of the listens</dd>
		</div>
	</dl>

	<figure class="mt-4">
		<div
			class="grid {columns} items-center gap-0.5"
			role="img"
			aria-label="Listens by weekday and hour; most on {weekdays[busiestDay]
				.long} and at {hourRange(busiestHour)}"
		>
			<!-- Listens per hour over all weekdays. -->
			<span></span>
			{#each hourTotals as n, hour (hour)}
				<div class="flex h-10 items-end" title="{hourRange(hour)}: {listenCount(n)}">
					<div
						class="w-full rounded-t-sm bg-stone-300"
						style:height="{n === 0 ? 0 : Math.max(6, (n / hourMax) * 100)}%"
					></div>
				</div>
			{/each}
			<span class="hidden sm:block"></span>

			{#each rows as i (i)}
				{@const hours = clock.weekdays[i]}
				<span class="pr-1.5 text-xs text-stone-500">{weekdays[i].short}</span>
				{#each hours as n, hour (hour)}
					<div
						class="aspect-square rounded-sm {shade(n)}"
						title="{weekdays[i].long}, {hourRange(hour)}: {listenCount(n)}"
					></div>
				{/each}
				<div
					class="hidden h-full items-center pl-1.5 sm:flex"
					title="{weekdays[i].long}: {listenCount(dayTotals[i])}"
				>
					<div
						class="h-2/3 rounded-r-sm bg-stone-300"
						style:width="{(dayTotals[i] / dayMax) * 100}%"
					></div>
				</div>
			{/each}

			<span></span>
			{#each axis as tick (tick.hour)}
				<span class="col-span-6 pt-0.5 text-xs text-stone-500">{tick.label}</span>
			{/each}
		</div>

		<figcaption class="mt-2 flex items-center gap-1 text-xs text-stone-500">
			Fewer
			<span class="size-2.5 rounded-sm bg-stone-200/60"></span>
			{#each shades as s (s)}
				<span class="size-2.5 rounded-sm {s}"></span>
			{/each}
			More listens
		</figcaption>
	</figure>
</section>
