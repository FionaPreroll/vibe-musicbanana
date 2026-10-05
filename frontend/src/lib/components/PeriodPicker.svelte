<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { listenCount } from '#lib/format.ts';
	import {
		isoDay,
		periodDays,
		periodLabel,
		periodSearch,
		recentDays,
		samePeriod,
		type Period
	} from '#lib/period.ts';
	import { sourceOf, withSource } from '#lib/source.ts';

	let {
		period,
		years,
		first,
		last
	}: {
		period: Period;
		/** Listens per year, including the years without any. */
		years: { year: number; listens: number }[];
		/** The profile's first and last listen, to start the custom period from. */
		first: string | null;
		last: string | null;
	} = $props();

	const presets: Period[] = [
		{ kind: 'all' },
		...recentDays.map((days): Period => ({ kind: 'days', days }))
	];
	const href = (p: Period) => page.url.pathname + withSource(periodSearch(p), sourceOf(page.url));
	const busiestYear = $derived(Math.max(1, ...years.map((y) => y.listens)));

	// The form for any other period opens on demand and stays open while it names the period.
	let custom = $derived(period.kind === 'range');
	// It starts with the days shown, or with all of them.
	const days = $derived.by(() => {
		const { from, to } = periodDays(period);
		return {
			from: from ?? (first && isoDay(new Date(first))),
			to: to ?? (last && isoDay(new Date(last)))
		};
	});

	// Leaves an empty date out of the URL rather than sending it as ?to=.
	function show(event: SubmitEvent) {
		event.preventDefault();
		const form = new FormData(event.currentTarget as HTMLFormElement);
		const day = (name: string) => String(form.get(name) ?? '') || null;
		const [from, to] = [day('from'), day('to')];
		const chosen: Period = from || to ? { kind: 'range', from, to } : { kind: 'all' };
		goto(href(chosen), { reset: false });
	}

	const button = (selected: boolean) =>
		`shrink-0 rounded px-2 py-1 text-sm whitespace-nowrap ${
			selected ? 'bg-stone-900 text-white' : 'text-stone-600 hover:bg-stone-200'
		}`;

	// On narrow screens the years scroll sideways; keep the selected one in view.
	function reveal(node: HTMLElement) {
		node.scrollIntoView({ block: 'nearest', inline: 'nearest' });
	}
</script>

<nav class="mt-8" aria-label="Period">
	<div class="flex flex-wrap gap-1">
		{#each presets as preset (periodSearch(preset))}
			{@const selected = samePeriod(preset, period)}
			<a
				href={href(preset)}
				data-sveltekit-noscroll
				class={button(selected)}
				aria-current={selected ? 'page' : undefined}>{periodLabel(preset)}</a
			>
		{/each}
		<button
			type="button"
			class={button(period.kind === 'range')}
			aria-expanded={custom}
			aria-controls="custom-period"
			onclick={() => (custom = !custom)}
		>
			{period.kind === 'range' ? periodLabel(period) : 'Custom'}
		</button>
	</div>

	{#if custom}
		<form
			id="custom-period"
			method="GET"
			onsubmit={show}
			class="mt-3 flex flex-wrap items-end gap-2 text-sm"
		>
			<label class="flex flex-col gap-0.5 text-stone-600">
				From
				<input
					type="date"
					name="from"
					value={days.from}
					class="rounded border-stone-300 py-1 text-sm text-stone-900"
				/>
			</label>
			<label class="flex flex-col gap-0.5 text-stone-600">
				To
				<input
					type="date"
					name="to"
					value={days.to}
					class="rounded border-stone-300 py-1 text-sm text-stone-900"
				/>
			</label>
			<button type="submit" class="rounded bg-stone-900 px-3 py-1.5 text-white hover:bg-stone-700">
				Show
			</button>
		</form>
	{/if}

	{#if years.length > 0}
		<div class="mt-4 flex min-w-0 items-end gap-1 overflow-x-auto">
			{#each years as y (y.year)}
				{@const selected = period.kind === 'year' && period.year === y.year}
				<a
					href={href({ kind: 'year', year: y.year })}
					data-sveltekit-noscroll
					class="group flex w-10 shrink-0 flex-col items-center"
					title={listenCount(y.listens)}
					aria-current={selected ? 'page' : undefined}
					{@attach selected && reveal}
				>
					<span class="flex h-16 w-8 items-end">
						<span
							class="w-full rounded-t {selected
								? 'bg-yellow-500'
								: 'bg-yellow-200 group-hover:bg-yellow-300'}"
							style:height="{y.listens && Math.max(4, (y.listens / busiestYear) * 100)}%"
						></span>
					</span>
					<span
						class="mt-1 text-xs tabular-nums {selected
							? 'font-semibold text-stone-900'
							: 'text-stone-500'}">{y.year}</span
					>
				</a>
			{/each}
		</div>
	{/if}
</nav>
