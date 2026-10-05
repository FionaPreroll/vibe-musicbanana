<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import { profilePath } from '#lib/api.ts';
	import { connectionState } from '#lib/connections.ts';
	import { formatDateTime, formatNumber, listenCount } from '#lib/format.ts';
	import { sourceLabel } from '#lib/source.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const s = $derived(data.status);

	// Fresh numbers every 10 seconds while the page is open.
	$effect(() => {
		const timer = setInterval(() => invalidateAll(), 10_000);
		return () => clearInterval(timer);
	});

	function bytes(n: number | null) {
		if (n == null) return '–';
		const units = ['B', 'KB', 'MB', 'GB', 'TB'];
		let i = 0;
		while (n >= 1024 && i < units.length - 1) {
			n /= 1024;
			i++;
		}
		return `${n.toFixed(i === 0 || n >= 100 ? 0 : 1)} ${units[i]}`;
	}

	function duration(seconds: number) {
		const d = Math.floor(seconds / 86400);
		const h = Math.floor((seconds % 86400) / 3600);
		const m = Math.floor((seconds % 3600) / 60);
		return d > 0 ? `${d} d ${h} h` : h > 0 ? `${h} h ${m} min` : `${m} min ${seconds % 60} s`;
	}

	const ms = (n: number) => (n < 0.1 ? '< 0.1 ms' : `${n.toFixed(n < 10 ? 1 : 0)} ms`);

	const heading = 'mb-3 text-sm font-semibold tracking-wide text-stone-500 uppercase';
	const term = 'text-stone-500';
	const cell = 'py-1.5 pr-4';
	const num = `${cell} text-right whitespace-nowrap tabular-nums`;
</script>

<svelte:head>
	<title>Status · musicbanana</title>
</svelte:head>

{#snippet facts(rows: [string, string][])}
	<dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1 text-sm">
		{#each rows as [name, value] (name)}
			<dt class={term}>{name}</dt>
			<dd class="min-w-0 break-words">{value}</dd>
		{/each}
	</dl>
{/snippet}

<main class="mx-auto max-w-4xl px-4 pb-16 sm:px-8">
	<h1 class="mt-6 text-3xl font-bold">Status</h1>
	<p class="mt-1 text-sm text-stone-600">Refreshes every 10 seconds.</p>

	<div class="mt-8 grid gap-10 md:grid-cols-2">
		<section>
			<h2 class={heading}>Build</h2>
			{@render facts([
				['Version', s.build.version + (s.build.debug ? ' (debug build)' : '')],
				['Commit', s.build.commit ? s.build.commit.slice(0, 12) : 'not known (local build)'],
				['Started', formatDateTime(s.build.started_at)],
				['Uptime', duration(s.build.uptime_seconds)]
			])}
		</section>

		<section>
			<h2 class={heading}>Process</h2>
			{@render facts([
				[
					'CPU',
					s.process.cpu_percent == null
						? '–'
						: `${s.process.cpu_percent} % of a core (${formatNumber(Math.round(s.process.cpu_seconds ?? 0))} s in all)`
				],
				[
					'Memory',
					`${bytes(s.process.memory_bytes)}, at most ${bytes(s.process.memory_peak_bytes)}`
				],
				['Threads', s.process.threads == null ? '–' : String(s.process.threads)],
				[
					'Machine',
					`${s.process.cores} cores, load ${s.process.load_average?.map((l) => l.toFixed(2)).join(' ') ?? '–'}`
				],
				[
					'Machine memory',
					`${bytes(s.process.memory_available_bytes)} free of ${bytes(s.process.memory_total_bytes)}` +
						(s.process.memory_limit_bytes ? `, limit ${bytes(s.process.memory_limit_bytes)}` : '')
				]
			])}
		</section>

		<section>
			<h2 class={heading}>Database</h2>
			{@render facts([
				['PostgreSQL', s.database.version.split(' on ')[0]],
				['Size', bytes(s.database.bytes)],
				['Migration', s.database.migration == null ? '–' : String(s.database.migration)],
				[
					'Connections',
					`${s.database.pool_size - s.database.pool_idle} busy, ${s.database.pool_idle} idle in the pool; ${s.database.connections ?? '–'} to the database`
				],
				[
					'Cache hits',
					s.database.cache_hit_percent == null ? '–' : `${s.database.cache_hit_percent} %`
				]
			])}
		</section>

		<section>
			<h2 class={heading}>Listens in the last 24 hours</h2>
			{#if s.database.last_day.length === 0}
				<p class="text-sm text-stone-500">None.</p>
			{:else}
				{@render facts(
					s.database.last_day.map((r) => [sourceLabel(r.source), listenCount(r.listens)])
				)}
			{/if}
		</section>
	</div>

	<section class="mt-10">
		<h2 class={heading}>YourSpotify connections</h2>
		<p class="mb-2 text-sm text-stone-500">
			The server last looked for due connections {s.workers.last_round_at
				? `at ${formatDateTime(s.workers.last_round_at)}`
				: 'not yet'}.
		</p>
		{#if s.workers.yourspotify.length === 0}
			<p class="text-sm text-stone-500">None.</p>
		{:else}
			<ul class="divide-y divide-stone-200 text-sm">
				{#each s.workers.yourspotify as c (c.id)}
					<li class="py-2">
						<span class="text-stone-500 tabular-nums">{c.id}</span>
						<a class="ml-2 font-medium hover:underline" href={profilePath(c.username, c.profile)}
							>{c.username}/{c.profile}</a
						>
						<span class="ml-2 text-stone-500">{c.url}</span>
						<span class="ml-2 text-stone-500 tabular-nums">{listenCount(c.imported)}</span>
						<p class={c.error ? 'text-red-700' : 'text-stone-500'}>{connectionState(c)}</p>
					</li>
				{/each}
			</ul>
		{/if}
	</section>

	<section class="mt-10">
		<h2 class={heading}>API response times since the start</h2>
		<p class="mb-2 text-sm text-stone-500">
			By route under /api, the ones that took the most time in all first; median and 95th percentile
			of the latest 500 requests each.
		</p>
		<div class="overflow-x-auto">
			<table class="w-full text-sm">
				<thead class="text-left text-stone-500">
					<tr>
						<th class={cell}>Route</th>
						<th class="{num} font-normal">Requests</th>
						<th class="{num} font-normal">5xx</th>
						<th class="{num} font-normal">Mean</th>
						<th class="{num} font-normal">Median</th>
						<th class="{num} font-normal">95 %</th>
						<th class="{num} font-normal">Max</th>
					</tr>
				</thead>
				<tbody class="divide-y divide-stone-200">
					{#each s.routes as r (r.route)}
						<tr>
							<td class="{cell} min-w-56 font-mono text-xs break-all"
								>{r.route.replace(' /api/', ' /')}</td
							>
							<td class={num}>{formatNumber(r.requests)}</td>
							<td class="{num} {r.server_errors ? 'text-red-700' : ''}">{r.server_errors}</td>
							<td class={num}>{ms(r.mean_ms)}</td>
							<td class={num}>{ms(r.p50_ms)}</td>
							<td class={num}>{ms(r.p95_ms)}</td>
							<td class={num}>{ms(r.max_ms)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	</section>

	<section class="mt-10">
		<h2 class={heading}>Tables</h2>
		<table class="text-sm">
			<thead class="text-left text-stone-500">
				<tr>
					<th class="{cell} font-normal">Table</th>
					<th class="{num} font-normal">Rows (about)</th>
					<th class="{num} font-normal">Size</th>
				</tr>
			</thead>
			<tbody class="divide-y divide-stone-200">
				{#each s.database.tables as t (t.name)}
					<tr>
						<td class="{cell} font-mono text-xs">{t.name}</td>
						<td class={num}>{formatNumber(t.rows)}</td>
						<td class={num}>{bytes(t.bytes)}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</section>
</main>
