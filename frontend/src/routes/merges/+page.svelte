<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import {
		errorMessage,
		getJson,
		sendJson,
		type CatalogEntry,
		type CatalogKind,
		type HiddenSuggestion,
		type MergeLog,
		type MergeOp,
		type MergeResult,
		type MergeSuggestions,
		type UndoResult
	} from '#lib/api.ts';
	import CatalogPicker from '#lib/components/CatalogPicker.svelte';
	import { formatDateTime, listenCount } from '#lib/format.ts';
	import Fill from '#lib/i18n/Fill.svelte';
	import { m } from '#lib/i18n/index.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const kindLabel = (kind: CatalogKind) => m.merges.kind[kind];
	const kinds: CatalogKind[] = ['artist', 'release', 'recording'];
	const likeness = (l: MergeSuggestions['suggestions'][number]['likeness']) => m.merges.likeness[l];

	let busy = $state(false);
	let error = $state('');
	// What the latest action did, with the merge to undo it by.
	let done: { message: string; undo?: number } | null = $state(null);

	// The merge being looked at: its dry run, where on the page it was asked for
	// (a suggestion's "from-into" or "manual"), and whether to merge in spite of
	// MusicBrainz IDs.
	let pending: { at: string; preview: MergeResult; force: boolean } | null = $state(null);

	// Merge by hand.
	let manualFrom: CatalogEntry | null = $state(null);
	let manualInto: CatalogEntry | null = $state(null);
	$effect(() => {
		void data.kind;
		manualFrom = null;
		manualInto = null;
		pending = null;
		hiddenShown = null;
	});

	// The log: the pages loaded so far; "Show more" adds the next one.
	let log: MergeOp[] = $derived([...data.log.merges]);
	let next: number | null = $derived(data.log.next);
	let undoing: UndoResult | null = $state(null);

	let hiddenShown: HiddenSuggestion[] | null = $state(null);

	async function act(action: () => Promise<string | { message: string; undo?: number }>) {
		busy = true;
		error = '';
		done = null;
		try {
			const result = await action();
			done = typeof result === 'string' ? { message: result } : result;
			await invalidateAll();
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	async function preview(at: string, kind: CatalogKind, from: number, into: number) {
		busy = true;
		error = '';
		done = null;
		undoing = null;
		try {
			const result = await sendJson<MergeResult>(fetch, 'POST', '/api/admin/merges', {
				kind,
				from,
				into,
				dry_run: true
			});
			pending = { at, preview: result, force: false };
		} catch (e) {
			pending = null;
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	function merge() {
		const p = pending!;
		act(async () => {
			const result = await sendJson<MergeResult>(fetch, 'POST', '/api/admin/merges', {
				kind: p.preview.kind,
				from: p.preview.from[0],
				into: p.preview.into[0],
				force: p.preview.told_apart && p.force
			});
			pending = null;
			if (p.at === 'manual') {
				manualFrom = null;
				manualInto = null;
			}
			return {
				message: m.merges.merged(
					kindLabel(result.kind),
					result.from[1],
					result.into[1],
					result.listens
				),
				undo: result.op ?? undefined
			};
		});
	}

	function hide(kind: CatalogKind, from: number, into: number) {
		act(async () => {
			await sendJson(fetch, 'POST', `/api/admin/merges/hidden/${kind}`, { from, into });
			if (pending?.at === `${from}-${into}`) pending = null;
			return m.merges.hidden;
		});
	}

	async function toggleHidden() {
		if (hiddenShown) {
			hiddenShown = null;
			return;
		}
		try {
			hiddenShown = await getJson<HiddenSuggestion[]>(
				fetch,
				`/api/admin/merges/hidden/${data.kind}`
			);
		} catch (e) {
			error = errorMessage(e);
		}
	}

	function unhide(h: HiddenSuggestion) {
		act(async () => {
			await sendJson(
				fetch,
				'DELETE',
				`/api/admin/merges/hidden/${data.kind}/${h.from[0]}/${h.into[0]}`
			);
			hiddenShown = hiddenShown?.filter((x) => x !== h) ?? null;
			return m.merges.suggestedAgain(h.from[1], h.into[1]);
		});
	}

	async function showMore() {
		if (!next) return;
		busy = true;
		try {
			const more = await getJson<MergeLog>(fetch, '/api/admin/merges', { before: next });
			log = [...log, ...more.merges];
			next = more.next;
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	async function previewUndo(op: number) {
		busy = true;
		error = '';
		done = null;
		pending = null;
		try {
			undoing = await sendJson<UndoResult>(fetch, 'POST', `/api/admin/merges/${op}/undo`, {
				dry_run: true
			});
		} catch (e) {
			undoing = null;
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	function undo(op: number) {
		act(async () => {
			const result = await sendJson<UndoResult>(fetch, 'POST', `/api/admin/merges/${op}/undo`, {});
			undoing = null;
			const undone = result.merge;
			return m.merges.undid(undone.id, kindLabel(undone.kind), undone.from[1]);
		});
	}

	const button =
		'rounded border border-stone-300 bg-white px-3 py-1.5 text-sm hover:bg-stone-100 disabled:opacity-50';
	const small =
		'rounded border border-stone-300 bg-white px-2 py-0.5 text-xs hover:bg-stone-100 disabled:opacity-50';
	const heading = 'mb-3 text-sm font-semibold tracking-wide text-stone-500 uppercase';
	const tab = 'border-b-2 px-1 pb-2 text-sm font-medium';
</script>

<svelte:head>
	<title>{m.merges.title} · musicbanana</title>
</svelte:head>

{#snippet entry(e: CatalogEntry)}
	<span class="font-medium">{e.name}</span>
	{#if e.artist}<span class="text-stone-500">· {e.artist}</span>{/if}
	<span class="text-stone-500 tabular-nums">· {listenCount(e.listens)}</span>
{/snippet}

{#snippet previewBox(p: MergeResult)}
	<div class="mt-2 rounded border border-yellow-300 bg-yellow-50 p-3 text-sm">
		<p>
			<Fill text={m.merges.wouldMerge(kindLabel(p.kind))}>
				{#snippet from()}<span class="font-medium">{m.common.quote(p.from[1])}</span>
					(#{p.from[0]}){/snippet}
				{#snippet into()}<span class="font-medium">{m.common.quote(p.into[1])}</span>
					(#{p.into[0]}){/snippet}
			</Fill>
		</p>
		<ul class="mt-1 list-inside list-disc text-stone-700">
			<li>{m.merges.countFor(p.listens, p.spellings, p.into[1])}</li>
			{#if p.kind === 'artist'}
				<li>{m.merges.albumsMoved(p.releases_moved, p.releases_merged)}</li>
				<li>{m.merges.tracksMoved(p.recordings_moved, p.recordings_merged)}</li>
			{/if}
		</ul>
		{#if p.told_apart}
			<label class="mt-2 flex items-start gap-2 text-red-800">
				<input class="mt-1" type="checkbox" bind:checked={pending!.force} />
				<span>{m.merges.toldApart}</span>
			</label>
		{/if}
		<div class="mt-3 flex gap-2">
			<button
				class="{button} border-yellow-500 font-medium"
				disabled={busy || (p.told_apart && !pending!.force)}
				onclick={merge}>{m.merges.merge}</button
			>
			<button class={button} disabled={busy} onclick={() => (pending = null)}
				>{m.common.cancel}</button
			>
		</div>
	</div>
{/snippet}

<main class="mx-auto max-w-4xl px-4 pb-16 sm:px-8">
	<h1 class="mt-6 text-3xl font-bold">{m.merges.title}</h1>
	<p class="mt-1 text-stone-600">{m.merges.intro}</p>

	<nav class="mt-8 flex gap-6 border-b border-stone-200">
		{#each kinds as kind (kind)}
			<a
				class="{tab} {data.kind === kind
					? 'border-stone-900'
					: 'border-transparent text-stone-500'}"
				href="?kind={kind}"
				aria-current={data.kind === kind ? 'page' : undefined}>{m.merges.kinds[kind]}</a
			>
		{/each}
	</nav>

	{#if error}<p class="mt-4 text-sm text-red-700" role="alert">{error}</p>{/if}
	{#if done}
		<p class="mt-4 text-sm text-green-800" role="status">
			{done.message}
			{#if done.undo}
				<button class="ml-1 underline" disabled={busy} onclick={() => previewUndo(done!.undo!)}
					>{m.common.undo}</button
				>
			{/if}
		</p>
	{/if}

	<section class="mt-6">
		<h2 class={heading}>{m.merges.suggestions}</h2>
		<p class="text-sm text-stone-600">
			{#if data.suggestions.total === 0}
				{m.merges.noneLeft}
			{:else if data.suggestions.total > data.suggestions.suggestions.length}
				{m.merges.firstOf(data.suggestions.suggestions.length, data.suggestions.total)}
			{:else}
				{m.merges.count(data.suggestions.total)}
			{/if}
			{#if data.kind !== 'artist'}
				{m.merges.withinArtist}
			{/if}
			{#if data.suggestions.hidden > 0}
				<button class="underline" onclick={toggleHidden}
					>{hiddenShown
						? m.merges.closeHidden
						: m.merges.hiddenCount(data.suggestions.hidden)}</button
				>
			{/if}
		</p>

		{#if hiddenShown}
			<ul
				class="mt-3 divide-y divide-stone-200 rounded border border-stone-200 bg-stone-50 text-sm"
			>
				{#each hiddenShown as h (`${h.from[0]}-${h.into[0]}`)}
					<li class="flex flex-wrap items-baseline gap-x-3 gap-y-1 px-3 py-2">
						<span class="min-w-56 flex-1"
							>{m.merges.pair(h.from[1], h.from[0], h.into[1], h.into[0])}
							<span class="text-stone-500">· {m.merges.hiddenAt(formatDateTime(h.hidden_at))}</span
							></span
						>
						<button class={small} disabled={busy} onclick={() => unhide(h)}
							>{m.merges.suggestAgain}</button
						>
					</li>
				{:else}
					<li class="px-3 py-2 text-stone-500">{m.common.none}</li>
				{/each}
			</ul>
		{/if}

		{#if data.suggestions.suggestions.length}
			<ul class="mt-3 divide-y divide-stone-200 text-sm">
				{#each data.suggestions.suggestions as s (`${s.from.id}-${s.into.id}`)}
					{@const at = `${s.from.id}-${s.into.id}`}
					{@const [label, why] = likeness(s.likeness)}
					<li class="py-2">
						<div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
							<span
								class="shrink-0 rounded px-1.5 py-0.5 text-xs {s.likeness === 'version'
									? 'bg-stone-100 text-stone-600'
									: 'bg-yellow-100 text-yellow-900'}"
								title={why}>{label}</span
							>
							<span class="min-w-56 flex-1">
								{@render entry(s.from)}
								<span class="text-stone-400" aria-label={m.merges.into}>→</span>
								{@render entry(s.into)}
							</span>
							<span class="flex shrink-0 gap-1.5">
								<button
									class={small}
									disabled={busy}
									onclick={() => preview(at, data.kind, s.from.id, s.into.id)}
									>{m.merges.mergeEllipsis}</button
								>
								<button
									class={small}
									disabled={busy}
									title={m.merges.otherWayTitle(s.from.name, s.into.name)}
									onclick={() => preview(at, data.kind, s.into.id, s.from.id)}
									>{m.merges.otherWay}</button
								>
								<button
									class={small}
									disabled={busy}
									title={m.merges.hideTitle}
									onclick={() => hide(data.kind, s.from.id, s.into.id)}>{m.merges.hide}</button
								>
							</span>
						</div>
						{#if pending?.at === at}{@render previewBox(pending.preview)}{/if}
					</li>
				{/each}
			</ul>
		{/if}
	</section>

	<section class="mt-10">
		<h2 class={heading}>{m.merges.byHand}</h2>
		<p class="text-sm text-stone-600">
			{m.merges.anyTwo(m.merges.kinds[data.kind])}
		</p>
		<div class="mt-3 flex flex-wrap items-end gap-3">
			<CatalogPicker kind={data.kind} label={m.merges.fromLabel} bind:picked={manualFrom} />
			<button
				class="{button} shrink-0"
				title={m.merges.swap}
				aria-label={m.merges.swap}
				disabled={busy}
				onclick={() => ([manualFrom, manualInto] = [manualInto, manualFrom])}>⇄</button
			>
			<CatalogPicker kind={data.kind} label={m.merges.intoLabel} bind:picked={manualInto} />
			<button
				class="{button} shrink-0"
				disabled={busy || !manualFrom || !manualInto}
				onclick={() => preview('manual', data.kind, manualFrom!.id, manualInto!.id)}
				>{m.merges.preview}</button
			>
		</div>
		{#if pending?.at === 'manual'}{@render previewBox(pending.preview)}{/if}
	</section>

	<section class="mt-10">
		<h2 class={heading}>{m.merges.log}</h2>
		<p class="text-sm text-stone-600">
			<Fill text={m.merges.logIntro}>
				{#snippet editions()}<code>merge editions</code>{/snippet}
			</Fill>
		</p>
		{#if log.length === 0}
			<p class="mt-3 text-sm text-stone-500">{m.merges.noMerges}</p>
		{:else}
			<ul class="mt-3 divide-y divide-stone-200 text-sm">
				{#each log as op (op.id)}
					<li class="py-2">
						<div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
							<span class="w-12 shrink-0 text-stone-500 tabular-nums">#{op.id}</span>
							<span class="min-w-56 flex-1 {op.undone_at ? 'text-stone-500' : ''}">
								<span class="text-stone-500">{kindLabel(op.kind)}</span>
								<span class={op.undone_at ? 'line-through' : 'font-medium'}
									>{m.common.quote(op.from[1])}</span
								>
								<span class="text-stone-500">#{op.from[0]}</span>
								<span class="text-stone-400" aria-label={m.merges.into}>→</span>
								<span class={op.undone_at ? 'line-through' : 'font-medium'}
									>{m.common.quote(op.into[1])}</span
								>
								<span class="text-stone-500">#{op.into[0]}</span>
							</span>
							<span class="shrink-0 text-stone-500 tabular-nums"
								>{formatDateTime(op.merged_at)}</span
							>
							<span class="w-28 shrink-0 text-right">
								{#if op.undone_at}
									<span class="text-xs text-stone-500" title={formatDateTime(op.undone_at)}
										>{m.merges.undone}</span
									>
								{:else}
									<button class={small} disabled={busy} onclick={() => previewUndo(op.id)}
										>{m.merges.undoEllipsis}</button
									>
								{/if}
							</span>
						</div>
						{#if undoing?.merge.id === op.id}
							<div class="mt-2 rounded border border-yellow-300 bg-yellow-50 p-3 text-sm">
								<p>
									{m.merges.wouldUndo(
										op.id,
										kindLabel(op.kind),
										op.from[1],
										undoing.restored,
										undoing.kept
									)}
								</p>
								<div class="mt-3 flex gap-2">
									<button
										class="{button} border-yellow-500 font-medium"
										disabled={busy}
										onclick={() => undo(op.id)}>{m.merges.undoMerge}</button
									>
									<button class={button} disabled={busy} onclick={() => (undoing = null)}
										>{m.common.cancel}</button
									>
								</div>
							</div>
						{/if}
					</li>
				{/each}
			</ul>
			{#if next}
				<button class="{button} mt-4" disabled={busy} onclick={showMore}>{m.common.showMore}</button
				>
			{/if}
		{/if}
	</section>
</main>
