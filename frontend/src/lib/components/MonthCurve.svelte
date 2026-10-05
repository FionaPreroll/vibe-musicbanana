<script lang="ts">
	import type { Month, Phase } from '#lib/api.ts';
	import { formatMonth, listenCount } from '#lib/format.ts';

	let { months, phases }: { months: Month[]; phases: Phase[] } = $props();

	const ordinal = (month: string) => {
		const [year, m] = month.split('-').map(Number);
		return year * 12 + m - 1;
	};

	// One slot per month. Where the API left out a year or more without listens,
	// a break of a few slots stands in for it.
	const layout = $derived.by(() => {
		const breakSlots = Math.max(2, Math.round(months.length / 40));
		const slots: number[] = [];
		const breaks: { slot: number; slots: number; after: string; before: string }[] = [];
		let next = 0;
		months.forEach((m, i) => {
			const previous = months[i - 1];
			if (previous && ordinal(m.month) - ordinal(previous.month) > 1) {
				breaks.push({ slot: next, slots: breakSlots, after: previous.month, before: m.month });
				next += breakSlots;
			}
			slots.push(next);
			next += 1;
		});
		return { slots, breaks, total: next };
	});

	const max = $derived(Math.max(1, ...months.map((m) => m.listens)));
	const index = $derived(new Map(months.map((m, i) => [m.month, i])));
	const bands = $derived(
		phases.map((p) => {
			const from = layout.slots[index.get(p.from) ?? 0];
			const to = layout.slots[index.get(p.to) ?? 0];
			return { from, length: to - from + 1 };
		})
	);
	const inPhase = (slot: number) => bands.some((b) => slot >= b.from && slot < b.from + b.length);

	// Bars of a few pixels need no gap, and a gap would blur them.
	const gap = $derived(layout.total > 120 ? 0 : 0.15);

	let width = $state(0);

	// A year label at the first month, after each break and at every January,
	// thinned out to every 2nd, 5th, … year when the years are too narrow for one each.
	// A label at the right end moves left to fit, and one after a break pushes out
	// a January label in its way. The next January in turn pushes out the label of
	// a year that starts late (Nov 2024 after a break, say), so the bars right of a
	// year's label belong to that year.
	const labels = $derived.by(() => {
		if (months.length === 0 || width === 0) return [];
		const perSlot = width / layout.total;
		const step = [1, 2, 5, 10, 20, 50].find((s) => 12 * s * perSlot >= 40) ?? 100;
		const labels: { slot: number; x: number; year: string; fixed: boolean; january: boolean }[] =
			[];
		months.forEach((m, i) => {
			const [year, month] = m.month.split('-');
			const slot = layout.slots[i];
			const fixed = i === 0 || slot !== layout.slots[i - 1] + 1;
			const january = month === '01';
			if (!fixed && (!january || Number(year) % step !== 0)) return;
			const x = Math.max(0, Math.min(slot * perSlot, width - 32));
			const crowded = () => {
				const previous = labels.at(-1);
				return previous !== undefined && x - previous.x < 40 ? previous : undefined;
			};
			while (fixed && crowded()?.fixed === false) labels.pop();
			const late = crowded();
			if (january && late && !late.january && Number(late.year) + 1 === Number(year)) labels.pop();
			if (crowded()) return;
			labels.push({ slot, x, year, fixed, january });
		});
		return labels;
	});
</script>

<div bind:clientWidth={width}>
	{#if months.length > 0}
		<svg
			class="block h-32 w-full"
			viewBox="0 0 {layout.total} 100"
			preserveAspectRatio="none"
			role="img"
			aria-label="Listens per month from {formatMonth(months[0].month)} to {formatMonth(
				months[months.length - 1].month
			)}"
		>
			{#each bands as band (band.from)}
				<rect class="fill-yellow-100" x={band.from} width={band.length} y="0" height="100" />
			{/each}
			{#each labels as label (label.slot)}
				<line
					class="stroke-stone-200"
					x1={label.slot}
					x2={label.slot}
					y1="0"
					y2="100"
					vector-effect="non-scaling-stroke"
				/>
			{/each}
			{#each layout.breaks as b (b.slot)}
				{#each [0.35, 0.65] as at (at)}
					<line
						class="stroke-stone-400"
						stroke-dasharray="3 3"
						x1={b.slot + b.slots * at}
						x2={b.slot + b.slots * at}
						y1="0"
						y2="100"
						vector-effect="non-scaling-stroke"
					/>
				{/each}
				<rect class="fill-transparent" x={b.slot} width={b.slots} y="0" height="100">
					<title>No listens between {formatMonth(b.after)} and {formatMonth(b.before)}</title>
				</rect>
			{/each}
			{#each months as m, i (m.month)}
				{#if m.listens > 0}
					{@const height = Math.max(2, (m.listens / max) * 100)}
					<rect
						class={inPhase(layout.slots[i]) ? 'fill-yellow-500' : 'fill-yellow-300'}
						x={layout.slots[i] + gap / 2}
						width={1 - gap}
						y={100 - height}
						{height}
					/>
				{/if}
			{/each}
			{#each months as m, i (m.month)}
				<rect
					class="fill-transparent hover:fill-stone-900/10"
					x={layout.slots[i]}
					width="1"
					y="0"
					height="100"
				>
					<title>{formatMonth(m.month)}: {listenCount(m.listens)}</title>
				</rect>
			{/each}
		</svg>
		<div class="relative h-5 border-t border-stone-300" aria-hidden="true">
			{#each labels as label (label.slot)}
				<span
					class="absolute top-0.5 pl-0.5 text-xs text-stone-500 tabular-nums"
					style:left="{label.x}px">{label.year}</span
				>
			{/each}
		</div>
	{/if}
</div>
