<script lang="ts">
	import { entityPath, type YearTop } from '#lib/api.ts';
	import { formatPercent, listenCount } from '#lib/format.ts';

	let {
		years,
		base,
		selected = null
	}: {
		years: YearTop[];
		/** The profile page; the artist pages are below it. */
		base: string;
		/** A year to stand out, e.g. the one the charts above show. */
		selected?: number | null;
	} = $props();

	// Each year is a column of its top artists, best first. In the gutter between two
	// consecutive years a line joins the two places of an artist who is in both; a wider
	// gap with a break stands for years without listens. Sizes in pixels:
	const ROW = 28; // from one name to the next
	const NAME = 24; // height of a name
	const HEAD = 28; // the year above a column
	const NARROWEST = 80; // narrower columns would cut too many names; scroll instead
	const WIDEST = 168;
	const SCROLLING = 112;
	// Roomy gutters where the columns fit with them, else tight ones.
	const SPACINGS = [
		{ gutter: 20, gap: 36 },
		{ gutter: 14, gap: 28 }
	];

	// Six hues of the reference palette whose pairs all stay apart with normal color
	// vision (checked with the dataviz palette validator against the page background).
	// With color blindness some pairs come close; the names carry who is who.
	const palette = ['#2a78d6', '#1baf7a', '#eda100', '#008300', '#4a3aa7', '#e34948'];

	let width = $state(0);
	let scroller: HTMLElement | undefined = $state();

	const layout = $derived.by(() => {
		const breaks = years.map((y, i) => i > 0 && y.year - years[i - 1].year > 1);
		const spaceWith = (spacing: (typeof SPACINGS)[number], i: number) =>
			breaks[i] ? spacing.gap : spacing.gutter;
		const columnWith = (spacing: (typeof SPACINGS)[number]) => {
			let gutters = 0;
			for (let i = 1; i < years.length; i++) gutters += spaceWith(spacing, i);
			return Math.min(WIDEST, Math.floor((width - gutters) / years.length));
		};
		const fitting = SPACINGS.find((spacing) => columnWith(spacing) >= NARROWEST);
		const spacing = fitting ?? SPACINGS[0];
		const column = fitting ? columnWith(fitting) : SCROLLING;
		const xs: number[] = [];
		for (let i = 0; i < years.length; i++) {
			xs.push(i === 0 ? 0 : xs[i - 1] + column + spaceWith(spacing, i));
		}
		const rows = Math.max(1, ...years.map((y) => y.artists.length));
		return {
			breaks,
			column,
			gutter: spacing.gutter,
			xs,
			rows,
			width: (xs.at(-1) ?? 0) + column,
			height: HEAD + (rows - 1) * ROW + NAME
		};
	});

	const top = (rank: number) => HEAD + rank * ROW;
	const middle = (rank: number) => top(rank) + NAME / 2;

	// The artists in the chart for the most years get a color, if they are in it for two at least.
	const colors = $derived.by(() => {
		const stays = new Map<number, { name: string; years: number; listens: number }>();
		for (const y of years) {
			for (const a of y.artists) {
				const stay = stays.get(a.id) ?? { name: a.name, years: 0, listens: 0 };
				stay.years += 1;
				stay.listens += a.listens;
				stays.set(a.id, stay);
			}
		}
		const longest = [...stays]
			.filter(([, s]) => s.years >= 2)
			.sort(
				([, a], [, b]) => b.years - a.years || b.listens - a.listens || a.name.localeCompare(b.name)
			)
			.slice(0, palette.length);
		return new Map(longest.map(([id, s], i) => [id, { name: s.name, color: palette[i] }]));
	});

	const lines = $derived.by(() => {
		const lines: { id: number; d: string }[] = [];
		years.forEach((y, i) => {
			if (i === 0 || layout.breaks[i]) return;
			const before = new Map(years[i - 1].artists.map((a, rank) => [a.id, rank]));
			// Start and end under the names, so that their rounded corners leave no gap.
			const x0 = layout.xs[i - 1] + layout.column - 2;
			const x1 = layout.xs[i] + 2;
			const xm = (x0 + x1) / 2;
			y.artists.forEach((a, rank) => {
				const from = before.get(a.id);
				if (from === undefined) return;
				const [y0, y1] = [middle(from), middle(rank)];
				lines.push({ id: a.id, d: `M${x0} ${y0}C${xm} ${y0} ${xm} ${y1} ${x1} ${y1}` });
			});
		});
		// Colored lines over the gray ones.
		return lines.sort((a, b) => Number(colors.has(a.id)) - Number(colors.has(b.id)));
	});

	// The artist under the pointer or keyboard focus, and where, for the tooltip.
	let active: { id: number; column?: number; rank?: number } | null = $state(null);
	const dimmed = (id: number) => active !== null && active.id !== id;
	const colorOf = (id: number) => colors.get(id)?.color ?? 'var(--color-stone-300)';

	// Beside the name, towards the middle of the chart, so that it never sticks out.
	const tip = $derived.by(() => {
		if (active?.column === undefined || active.rank === undefined) return null;
		const year = years[active.column];
		const artist = year?.artists[active.rank];
		if (!artist) return null;
		const left = active.column >= years.length / 2;
		const x = layout.xs[active.column];
		return {
			year,
			artist,
			rank: active.rank,
			x: left ? x - 6 : x + layout.column + 6,
			y: middle(active.rank),
			shift: `translate(${left ? '-100%' : '0'}, -50%)`
		};
	});

	// When the chart scrolls, show the selected year or else the latest one.
	$effect(() => {
		if (!scroller || layout.width <= width) return;
		const i = selected === null ? years.length - 1 : years.findIndex((y) => y.year === selected);
		if (i < 0) return;
		const x = layout.xs[i];
		const left = scroller.scrollLeft;
		if (x < left) scroller.scrollLeft = x - layout.gutter;
		else if (x + layout.column > left + width)
			scroller.scrollLeft = x + layout.column - width + layout.gutter;
	});

	const gap = (i: number) => {
		const [after, before] = [years[i - 1].year + 1, years[i].year - 1];
		return after === before ? `No listens in ${after}` : `No listens from ${after} to ${before}`;
	};
</script>

<section>
	<h2 class="text-sm font-semibold tracking-wide text-stone-500 uppercase">
		Top artists over the years
	</h2>
	<p class="mt-1 text-sm text-stone-500">
		The most heard artists of each year. A line follows an artist from one year to the next.
	</p>

	{#if colors.size > 0}
		<div class="mt-3 flex flex-wrap items-baseline gap-x-4 gap-y-1 text-sm">
			<span class="text-stone-500">Longest at the top:</span>
			{#each colors as [id, { name, color }] (id)}
				<a
					href={entityPath(base, 'artist', id, name)}
					class="flex items-center gap-1.5 hover:underline"
					onpointerenter={() => (active = { id })}
					onpointerleave={() => (active = null)}
					onfocus={() => (active = { id })}
					onblur={() => (active = null)}
				>
					<span class="size-2.5 shrink-0 rounded-full" style:background-color={color}></span>
					{name}
				</a>
			{/each}
		</div>
	{/if}

	<div class="mt-4 flex">
		<ol class="relative w-7 shrink-0" style:height="{layout.height}px" aria-hidden="true">
			{#each { length: layout.rows }, rank}
				<li
					class="absolute right-2 flex h-6 items-center text-xs text-stone-400 tabular-nums"
					style:top="{top(rank)}px"
				>
					{rank + 1}
				</li>
			{/each}
		</ol>

		<div
			bind:this={scroller}
			bind:clientWidth={width}
			class="min-w-0 flex-1 overflow-x-auto pb-2"
			role="region"
			aria-label="Top artists of each year"
		>
			<div class="relative" style:width="{layout.width}px" style:height="{layout.height + 4}px">
				{#each years as y, i (y.year)}
					{#if y.year === selected}
						<div
							class="absolute -top-1 rounded-md bg-yellow-100"
							style:left="{layout.xs[i] - 4}px"
							style:width="{layout.column + 8}px"
							style:height="{layout.height + 8}px"
						></div>
					{/if}
				{/each}

				<svg
					class="absolute inset-0"
					width={layout.width}
					height={layout.height}
					aria-hidden="true"
				>
					{#each layout.breaks as isBreak, i}
						{#if isBreak}
							{@const x0 = layout.xs[i - 1] + layout.column}
							{@const x1 = layout.xs[i]}
							{#each [0.35, 0.65] as at (at)}
								<line
									class="stroke-stone-400"
									stroke-dasharray="3 3"
									x1={x0 + (x1 - x0) * at}
									x2={x0 + (x1 - x0) * at}
									y1={HEAD}
									y2={layout.height}
								/>
							{/each}
							<rect class="fill-transparent" x={x0} width={x1 - x0} y={HEAD} height={layout.height}>
								<title>{gap(i)}</title>
							</rect>
						{/if}
					{/each}
					{#each lines as line (line.d)}
						<path
							class="pointer-events-none fill-none transition-opacity"
							d={line.d}
							style:stroke={colorOf(line.id)}
							stroke-width="2"
							opacity={dimmed(line.id) ? 0.15 : 1}
						/>
					{/each}
					{#if active}
						{#each lines.filter((l) => l.id === active?.id) as line (line.d)}
							<path
								class="pointer-events-none fill-none"
								d={line.d}
								style:stroke={colors.has(line.id) ? colorOf(line.id) : 'var(--color-stone-500)'}
								stroke-width="3"
							/>
						{/each}
					{/if}
				</svg>

				{#each years as y, i (y.year)}
					<div class="absolute top-0" style:left="{layout.xs[i]}px" style:width="{layout.column}px">
						<a
							href="?year={y.year}"
							data-sveltekit-noscroll
							class="block h-6 text-center text-xs leading-6 tabular-nums {y.year === selected
								? 'font-semibold text-stone-900'
								: 'text-stone-500 hover:text-stone-900'}"
							title="Charts of {y.year}">{y.year}</a
						>
						<ol class="mt-1 space-y-1" aria-label="Top artists of {y.year}">
							{#each y.artists as artist, rank (artist.id)}
								<li>
									<a
										href={entityPath(base, 'artist', artist.id, artist.name)}
										class="flex h-6 items-center overflow-hidden rounded border bg-white text-xs transition-opacity {dimmed(
											artist.id
										)
											? 'border-stone-200 opacity-30'
											: active?.id === artist.id
												? 'border-stone-500'
												: 'border-stone-200 hover:border-stone-400'}"
										aria-label="{artist.name}, number {rank + 1} in {y.year}, {listenCount(
											artist.listens
										)}"
										onpointerenter={() => (active = { id: artist.id, column: i, rank })}
										onpointerleave={() => (active = null)}
										onfocus={() => (active = { id: artist.id, column: i, rank })}
										onblur={() => (active = null)}
									>
										<span class="h-full w-1 shrink-0" style:background-color={colorOf(artist.id)}
										></span>
										<span class="truncate px-1">{artist.name}</span>
									</a>
								</li>
							{/each}
						</ol>
					</div>
				{/each}

				{#if tip}
					<div
						class="pointer-events-none absolute z-10 rounded bg-stone-900 px-2 py-1 text-xs whitespace-nowrap text-white shadow"
						style:left="{tip.x}px"
						style:top="{tip.y}px"
						style:transform={tip.shift}
						aria-hidden="true"
					>
						<div class="font-medium">{tip.artist.name}</div>
						<div class="text-stone-300 tabular-nums">
							#{tip.rank + 1} in {tip.year.year} · {listenCount(tip.artist.listens)} · {formatPercent(
								tip.artist.listens / tip.year.listens
							)}
						</div>
					</div>
				{/if}
			</div>
		</div>
	</div>
</section>
