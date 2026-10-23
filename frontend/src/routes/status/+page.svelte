<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import { profilePath } from '#lib/api.ts';
	import { connectionState, refetchState } from '#lib/connections.ts';
	import { formatDateTime, formatNumber, listenCount } from '#lib/format.ts';
	import { localized } from '#lib/i18n/intl.ts';
	import { m } from '#lib/i18n/index.ts';
	import { sourceLabel } from '#lib/source.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const s = $derived(data.status);

	// Fresh numbers every 10 seconds while the page is open.
	$effect(() => {
		const timer = setInterval(() => invalidateAll(), 10_000);
		return () => clearInterval(timer);
	});

	// Numbers with as many decimals as given, in the page's locale.
	const decimals = [0, 1, 2].map((digits) =>
		localized(
			(locale) =>
				new Intl.NumberFormat(locale, {
					minimumFractionDigits: digits,
					maximumFractionDigits: digits
				})
		)
	);
	const fixed = (n: number, digits: number) => decimals[digits]().format(n);

	function bytes(n: number | null) {
		if (n == null) return '–';
		const units = ['B', 'KB', 'MB', 'GB', 'TB'];
		let i = 0;
		while (n >= 1024 && i < units.length - 1) {
			n /= 1024;
			i++;
		}
		return `${fixed(n, i === 0 || n >= 100 ? 0 : 1)} ${units[i]}`;
	}

	function duration(seconds: number) {
		const d = Math.floor(seconds / 86400);
		const h = Math.floor((seconds % 86400) / 3600);
		const min = Math.floor((seconds % 3600) / 60);
		return m.status.uptimeValue(d, h, min, seconds % 60);
	}

	const ms = (n: number) => (n < 0.1 ? `< ${fixed(0.1, 1)} ms` : `${fixed(n, n < 10 ? 1 : 0)} ms`);

	const heading = 'mb-3 text-sm font-semibold tracking-wide text-stone-500 uppercase';
	const term = 'text-stone-500';
	const cell = 'py-1.5 pr-4';
	const num = `${cell} text-right whitespace-nowrap tabular-nums`;
</script>

<svelte:head>
	<title>{m.status.title} · musicbanana</title>
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
	<h1 class="mt-6 text-3xl font-bold">{m.status.title}</h1>
	<p class="mt-1 text-sm text-stone-600">{m.status.refresh}</p>

	<div class="mt-8 grid gap-10 md:grid-cols-2">
		<section>
			<h2 class={heading}>{m.status.build}</h2>
			{@render facts([
				[m.status.version, s.build.version + (s.build.debug ? ` ${m.status.debugBuild}` : '')],
				[m.status.commit, s.build.commit ? s.build.commit.slice(0, 12) : m.status.unknownCommit],
				[m.status.started, formatDateTime(s.build.started_at)],
				[m.status.uptime, duration(s.build.uptime_seconds)]
			])}
		</section>

		<section>
			<h2 class={heading}>{m.status.process}</h2>
			{@render facts([
				[
					'CPU',
					s.process.cpu_percent == null
						? '–'
						: m.status.cpu(
								formatNumber(s.process.cpu_percent),
								formatNumber(Math.round(s.process.cpu_seconds ?? 0))
							)
				],
				[
					m.status.memory,
					m.status.memoryValue(bytes(s.process.memory_bytes), bytes(s.process.memory_peak_bytes))
				],
				[m.status.threads, s.process.threads == null ? '–' : formatNumber(s.process.threads)],
				[
					m.status.machine,
					m.status.machineValue(
						s.process.cores,
						s.process.load_average?.map((l) => fixed(l, 2)).join(' ') ?? '–'
					)
				],
				[
					m.status.machineMemory,
					m.status.free(
						bytes(s.process.memory_available_bytes),
						bytes(s.process.memory_total_bytes),
						s.process.memory_limit_bytes ? bytes(s.process.memory_limit_bytes) : null
					)
				]
			])}
		</section>

		<section>
			<h2 class={heading}>{m.status.database}</h2>
			{@render facts([
				['PostgreSQL', s.database.version.split(' on ')[0]],
				[m.status.size, bytes(s.database.bytes)],
				[m.status.migration, s.database.migration == null ? '–' : String(s.database.migration)],
				[
					m.status.connections,
					m.status.connectionsValue(
						s.database.pool_size - s.database.pool_idle,
						s.database.pool_idle,
						s.database.connections == null ? '–' : formatNumber(s.database.connections)
					)
				],
				[
					m.status.cacheHits,
					s.database.cache_hit_percent == null
						? '–'
						: `${formatNumber(s.database.cache_hit_percent)} %`
				]
			])}
		</section>

		<section>
			<h2 class={heading}>{m.status.lastDay}</h2>
			{#if s.database.last_day.length === 0}
				<p class="text-sm text-stone-500">{m.common.none}</p>
			{:else}
				{@render facts(
					s.database.last_day.map((r) => [sourceLabel(r.source), listenCount(r.listens)])
				)}
			{/if}
		</section>
	</div>

	<section class="mt-10">
		<h2 class={heading}>{m.status.yourspotify}</h2>
		<p class="mb-2 text-sm text-stone-500">
			{m.status.lastRound(s.workers.last_round_at ? formatDateTime(s.workers.last_round_at) : null)}
		</p>
		{#if s.workers.yourspotify.length === 0}
			<p class="text-sm text-stone-500">{m.common.none}</p>
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
						{#if refetchState(c)}
							<p class="text-stone-500">{refetchState(c)}</p>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	</section>

	<section class="mt-10">
		<h2 class={heading}>{m.status.routes}</h2>
		<p class="mb-2 text-sm text-stone-500">{m.status.routesIntro}</p>
		<div class="overflow-x-auto">
			<table class="w-full text-sm">
				<thead class="text-left text-stone-500">
					<tr>
						<th class={cell}>{m.status.route}</th>
						<th class="{num} font-normal">{m.status.requests}</th>
						<th class="{num} font-normal">5xx</th>
						<th class="{num} font-normal">{m.status.mean}</th>
						<th class="{num} font-normal">{m.status.median}</th>
						<th class="{num} font-normal">95 %</th>
						<th class="{num} font-normal">{m.status.max}</th>
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
		<h2 class={heading}>{m.status.tables}</h2>
		<table class="text-sm">
			<thead class="text-left text-stone-500">
				<tr>
					<th class="{cell} font-normal">{m.status.table}</th>
					<th class="{num} font-normal">{m.status.rows}</th>
					<th class="{num} font-normal">{m.status.size}</th>
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
