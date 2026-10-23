<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import { SvelteSet } from 'svelte/reactivity';
	import {
		errorMessage,
		getJson,
		profilePath,
		sendJson,
		type HistoryListen,
		type HistoryPage
	} from '#lib/api.ts';
	import ListenTime from '#lib/components/ListenTime.svelte';
	import { byDay, formatDateTime, formatDay, formatNumber } from '#lib/format.ts';
	import { now } from '#lib/now.svelte.ts';
	import { filterQuery } from '#lib/history.ts';
	import Fill from '#lib/i18n/Fill.svelte';
	import { m } from '#lib/i18n/index.ts';
	import { sourceLabel } from '#lib/source.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const page = $derived(data.trash ? data.inTrash : data.history!);
	// The pages loaded so far; "Show more" adds the next one.
	let shown: HistoryListen[] = $derived([...page.listens]);
	let next: string | null = $derived(page.next);
	const days = $derived(byDay(shown, (l) => l.listened_at));
	const selected = new SvelteSet<number>();
	let busy = $state(false);
	let error = $state('');
	// What the latest action did, and how to take it back.
	let done: { message: string; undo?: number[] } | null = $state(null);

	const filtered = $derived(Object.values(data.filter).some((v) => v !== ''));
	const allShownSelected = $derived(shown.length > 0 && shown.every((l) => selected.has(l.id)));

	// A new filter or tab starts a new selection.
	$effect(() => {
		void page;
		selected.clear();
	});

	async function showMore() {
		if (!next) return;
		busy = true;
		try {
			const more = data.trash
				? await getJson<HistoryPage>(fetch, `${data.api}/trash`, { before: next })
				: await getJson<HistoryPage>(fetch, `${data.api}/history`, {
						...filterQuery(data.filter),
						before: next
					});
			shown = [...shown, ...more.listens];
			next = more.next;
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	function toggleAll() {
		if (allShownSelected) selected.clear();
		else for (const l of shown) selected.add(l.id);
	}

	async function act(action: () => Promise<string | { message: string; undo?: number[] }>) {
		busy = true;
		error = '';
		done = null;
		try {
			const result = await action();
			done = typeof result === 'string' ? { message: result } : result;
			selected.clear();
			await invalidateAll();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	const toTrash = (body: object) =>
		act(async () => {
			const { ids } = await sendJson<{ ids: number[] }>(fetch, 'POST', `${data.api}/trash`, body);
			return { message: m.history.moved(ids.length), undo: ids };
		});

	function trashAllMatching() {
		if (!confirm(m.history.confirmMoveAll(page.total))) return;
		toTrash(filterQuery(data.filter));
	}

	const restore = (ids?: number[]) =>
		act(async () => {
			const { restored, kept } = await sendJson<{ restored: number; kept: number }>(
				fetch,
				'POST',
				`${data.api}/trash/restore`,
				{ ids }
			);
			return m.history.restored(restored, kept);
		});

	function deleteForGood(ids?: number[]) {
		if (!confirm(m.history.confirmDelete(ids?.length ?? data.inTrash.total))) return;
		act(async () => {
			const { deleted } = await sendJson<{ deleted: number }>(
				fetch,
				'POST',
				`${data.api}/trash/empty`,
				{ ids }
			);
			return m.history.deleted(deleted);
		});
	}

	const input = 'rounded border border-stone-300 bg-white px-2 py-1.5';
	const button =
		'rounded border border-stone-300 bg-white px-3 py-1.5 text-sm hover:bg-stone-100 disabled:opacity-50';
	const tab = 'border-b-2 px-1 pb-2 text-sm font-medium';
</script>

<svelte:head>
	<title>{m.history.title} · musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-4xl px-4 pb-16 sm:px-8">
	<p class="mt-6 text-sm">
		<a class="text-stone-600 underline" href="/settings">{m.settings.title}</a>
	</p>
	<h1 class="mt-2 text-3xl font-bold">{m.history.title}</h1>
	<p class="mt-1 text-stone-600">
		<Fill text={m.history.intro}>
			{#snippet profile()}<a
					class="underline"
					href={profilePath(data.me!.username, data.profile.slug)}
					>{profilePath(data.me!.username, data.profile.slug)}</a
				>{/snippet}
		</Fill>
	</p>

	<nav class="mt-8 flex gap-6 border-b border-stone-200">
		<a
			class="{tab} {data.trash ? 'border-transparent text-stone-500' : 'border-stone-900'}"
			href="?"
			aria-current={data.trash ? undefined : 'page'}>{m.history.listens}</a
		>
		<a
			class="{tab} {data.trash ? 'border-stone-900' : 'border-transparent text-stone-500'}"
			href="?trash"
			aria-current={data.trash ? 'page' : undefined}
			>{m.history.trash}
			<span class="text-stone-500 tabular-nums">{formatNumber(data.inTrash.total)}</span></a
		>
	</nav>

	{#if !data.trash}
		<form class="mt-4 flex flex-wrap items-end gap-3" method="GET">
			<label class="flex min-w-48 flex-1 flex-col gap-1 text-sm">
				<span>{m.history.words}</span>
				<input
					class={input}
					name="q"
					type="search"
					placeholder={m.history.placeholder}
					value={data.filter.q}
				/>
			</label>
			<label class="flex flex-col gap-1 text-sm">
				<span>{m.common.from}</span>
				<input class={input} name="from" type="datetime-local" value={data.filter.from} />
			</label>
			<label class="flex flex-col gap-1 text-sm">
				<span>{m.common.to}</span>
				<input class={input} name="to" type="datetime-local" value={data.filter.to} />
			</label>
			<label class="flex flex-col gap-1 text-sm">
				<span>{m.history.source}</span>
				<select class={input} name="source" value={data.filter.source}>
					<option value="">{m.source.all}</option>
					{#each data.sources as s (s.source)}
						<option value={s.source}>{sourceLabel(s.source)}</option>
					{/each}
				</select>
			</label>
			<button class={button}>{m.common.show}</button>
			{#if filtered}<a class="pb-1.5 text-sm text-stone-600 underline" href="?">{m.history.clear}</a
				>{/if}
		</form>
	{/if}

	<div class="mt-6 flex flex-wrap items-center gap-x-4 gap-y-2 text-sm">
		<span class="text-stone-600">
			{data.trash ? m.history.inTrash(page.total) : m.history.total(page.total, filtered)}
			{#if selected.size}{m.history.selected(selected.size)}{/if}
		</span>
		<span class="ml-auto flex flex-wrap gap-2">
			{#if data.trash}
				<button
					class={button}
					disabled={busy || !selected.size}
					onclick={() => restore([...selected])}>{m.history.restoreSelected}</button
				>
				<button class={button} disabled={busy || !page.total} onclick={() => restore()}
					>{m.history.restoreAll}</button
				>
				<button
					class="{button} text-red-800"
					disabled={busy || !selected.size}
					onclick={() => deleteForGood([...selected])}>{m.history.deleteSelected}</button
				>
				<button
					class="{button} text-red-800"
					disabled={busy || !page.total}
					onclick={() => deleteForGood()}>{m.history.emptyTrash}</button
				>
			{:else}
				<button
					class={button}
					disabled={busy || !selected.size}
					onclick={() => toTrash({ ids: [...selected] })}>{m.history.moveSelected}</button
				>
				{#if filtered && page.total > 0}
					<button class={button} disabled={busy} onclick={trashAllMatching}
						>{m.history.moveAll(page.total)}</button
					>
				{/if}
			{/if}
		</span>
	</div>

	{#if error}<p class="mt-3 text-sm text-red-700" role="alert">{error}</p>{/if}
	{#if done}
		<p class="mt-3 text-sm text-green-800" role="status">
			{done.message}
			{#if done.undo?.length}
				<button class="ml-1 underline" disabled={busy} onclick={() => restore(done!.undo)}
					>{m.common.undo}</button
				>
			{/if}
		</p>
	{/if}

	{#if shown.length === 0}
		<p class="mt-6 text-sm text-stone-500">
			{data.trash ? m.history.trashEmpty : m.history.noListens}
		</p>
	{:else}
		<div class="mt-4 overflow-x-auto">
			<table class="w-full text-sm">
				<thead class="text-left text-stone-500">
					<tr>
						<th class="w-8 py-2 pr-2">
							<input
								type="checkbox"
								aria-label={m.history.selectAll}
								checked={allShownSelected}
								onchange={toggleAll}
							/>
						</th>
						<th class="py-2 pr-4 font-normal">{m.history.time}</th>
						<th class="py-2 pr-4 font-normal">{m.history.listen}</th>
						<th class="py-2 pr-4 font-normal">{m.history.source}</th>
						{#if data.trash}<th class="py-2 font-normal">{m.history.trashed}</th>{/if}
					</tr>
				</thead>
				{#each days as day (day.key)}
					<tbody class="divide-y divide-stone-200">
						<tr>
							<th
								class="pt-5 pb-1 text-left font-semibold text-stone-700"
								colspan={data.trash ? 5 : 4}
								scope="rowgroup"
							>
								<time datetime={day.key}>{formatDay(day.date, now())}</time>
							</th>
						</tr>
						{#each day.items as l (l.id)}
							<tr class={selected.has(l.id) ? 'bg-yellow-50' : ''}>
								<td class="py-2 pr-2 align-top">
									<input
										type="checkbox"
										aria-label={m.history.select(l.artist, l.track, formatDateTime(l.listened_at))}
										checked={selected.has(l.id)}
										onchange={(e) =>
											e.currentTarget.checked ? selected.add(l.id) : selected.delete(l.id)}
									/>
								</td>
								<td class="py-2 pr-4 align-top whitespace-nowrap text-stone-600 tabular-nums">
									<ListenTime at={l.listened_at} />
								</td>
								<td class="py-2 pr-4 align-top">
									<span class="font-medium">{l.track}</span>
									<span class="text-stone-500"
										>· {[l.artist, l.album].filter(Boolean).join(' · ')}</span
									>
								</td>
								<td class="py-2 pr-4 align-top whitespace-nowrap text-stone-500"
									>{sourceLabel(l.source)}</td
								>
								{#if data.trash}
									<td class="py-2 align-top whitespace-nowrap text-stone-500">
										<ListenTime at={l.trashed_at!} ago />
									</td>
								{/if}
							</tr>
						{/each}
					</tbody>
				{/each}
			</table>
		</div>
		{#if next}
			<button class="{button} mt-4" disabled={busy} onclick={showMore}>{m.common.showMore}</button>
		{/if}
	{/if}
</main>
