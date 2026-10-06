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
	import { formatDateTime, formatNumber, listenCount } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const kindLabel: Record<CatalogKind, string> = {
		artist: 'artist',
		release: 'album',
		recording: 'track'
	};
	const tabs: [CatalogKind, string][] = [
		['artist', 'Artists'],
		['release', 'Albums'],
		['recording', 'Tracks']
	];
	const likeness: Record<MergeSuggestions['suggestions'][number]['likeness'], [string, string]> = {
		same_letters: [
			'Same letters',
			'Only case, accents, punctuation, spaces, a leading "The" or "&" for "and" differ.'
		],
		one_letter: ['One letter apart', 'One letter more, missing, different or swapped.'],
		version: [
			'Another version?',
			'The same title apart from a note such as "(Live)", "- Radio Edit" or "feat. …". Live versions and the original are better kept apart.'
		]
	};

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
				message: `Merged ${kindLabel[result.kind]} “${result.from[1]}” into “${result.into[1]}”: ${listenCount(result.listens)} moved over.`,
				undo: result.op ?? undefined
			};
		});
	}

	function hide(kind: CatalogKind, from: number, into: number) {
		act(async () => {
			await sendJson(fetch, 'POST', `/api/admin/merges/hidden/${kind}`, { from, into });
			if (pending?.at === `${from}-${into}`) pending = null;
			return 'Hidden. It stays out of the suggestions until you show it again.';
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
			return `“${h.from[1]}” and “${h.into[1]}” are suggested again.`;
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
			const m = result.merge;
			return `Undid merge ${m.id}: ${kindLabel[m.kind]} “${m.from[1]}” stands on its own again.`;
		});
	}

	const count = (n: number, one: string, many: string) =>
		`${formatNumber(n)} ${n === 1 ? one : many}`;

	const button =
		'rounded border border-stone-300 bg-white px-3 py-1.5 text-sm hover:bg-stone-100 disabled:opacity-50';
	const small =
		'rounded border border-stone-300 bg-white px-2 py-0.5 text-xs hover:bg-stone-100 disabled:opacity-50';
	const heading = 'mb-3 text-sm font-semibold tracking-wide text-stone-500 uppercase';
	const tab = 'border-b-2 px-1 pb-2 text-sm font-medium';
</script>

<svelte:head>
	<title>Merges · musicbanana</title>
</svelte:head>

{#snippet entry(e: CatalogEntry)}
	<span class="font-medium">{e.name}</span>
	{#if e.artist}<span class="text-stone-500">· {e.artist}</span>{/if}
	<span class="text-stone-500 tabular-nums">· {listenCount(e.listens)}</span>
{/snippet}

{#snippet previewBox(p: MergeResult)}
	<div class="mt-2 rounded border border-yellow-300 bg-yellow-50 p-3 text-sm">
		<p>
			Would merge {kindLabel[p.kind]} <span class="font-medium">“{p.from[1]}”</span> (#{p.from[0]})
			into <span class="font-medium">“{p.into[1]}”</span> (#{p.into[0]}).
		</p>
		<ul class="mt-1 list-inside list-disc text-stone-700">
			<li>
				{listenCount(p.listens)} and {count(p.spellings, 'spelling', 'spellings')} count for “{p
					.into[1]}” from then on.
			</li>
			{#if p.kind === 'artist'}
				<li>
					Albums: {formatNumber(p.releases_moved)} moved over, {formatNumber(p.releases_merged)} merged
					with one of the same title.
				</li>
				<li>
					Tracks: {formatNumber(p.recordings_moved)} moved over, {formatNumber(p.recordings_merged)} merged
					with one of the same title.
				</li>
			{/if}
		</ul>
		{#if p.told_apart}
			<label class="mt-2 flex items-start gap-2 text-red-800">
				<input class="mt-1" type="checkbox" bind:checked={pending!.force} />
				<span
					>Their MusicBrainz IDs say these are different ones. Merge anyway, for example an artist's
					other name or a track MusicBrainz lists twice.</span
				>
			</label>
		{/if}
		<div class="mt-3 flex gap-2">
			<button
				class="{button} border-yellow-500 font-medium"
				disabled={busy || (p.told_apart && !pending!.force)}
				onclick={merge}>Merge</button
			>
			<button class={button} disabled={busy} onclick={() => (pending = null)}>Cancel</button>
		</div>
	</div>
{/snippet}

<main class="mx-auto max-w-4xl px-4 pb-16 sm:px-8">
	<h1 class="mt-6 text-3xl font-bold">Merges</h1>
	<p class="mt-1 text-stone-600">
		Merging puts two entries of the catalog together that are the same under different spellings,
		for every profile. The listens keep what the players sent, later listens with either spelling
		count for the one that stays, and every merge can be undone in the log below.
	</p>

	<nav class="mt-8 flex gap-6 border-b border-stone-200">
		{#each tabs as [kind, label] (kind)}
			<a
				class="{tab} {data.kind === kind
					? 'border-stone-900'
					: 'border-transparent text-stone-500'}"
				href="?kind={kind}"
				aria-current={data.kind === kind ? 'page' : undefined}>{label}</a
			>
		{/each}
	</nav>

	{#if error}<p class="mt-4 text-sm text-red-700" role="alert">{error}</p>{/if}
	{#if done}
		<p class="mt-4 text-sm text-green-800" role="status">
			{done.message}
			{#if done.undo}
				<button class="ml-1 underline" disabled={busy} onclick={() => previewUndo(done!.undo!)}
					>Undo</button
				>
			{/if}
		</p>
	{/if}

	<section class="mt-6">
		<h2 class={heading}>Suggestions</h2>
		<p class="text-sm text-stone-600">
			{#if data.suggestions.total === 0}
				No look-alikes left.
			{:else if data.suggestions.total > data.suggestions.suggestions.length}
				The first {formatNumber(data.suggestions.suggestions.length)} of {formatNumber(
					data.suggestions.total
				)}, the most certain first.
			{:else}
				{count(data.suggestions.total, 'look-alike', 'look-alikes')}, the most certain first.
			{/if}
			{#if data.kind !== 'artist'}
				Albums and tracks are only compared within an artist, so merging look-alike artists first
				also merges their albums and tracks of the same title.
			{/if}
			{#if data.suggestions.hidden > 0}
				<button class="underline" onclick={toggleHidden}
					>{hiddenShown
						? 'Close the hidden ones'
						: `${formatNumber(data.suggestions.hidden)} hidden`}</button
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
							>“{h.from[1]}” (#{h.from[0]}) and “{h.into[1]}” (#{h.into[0]})
							<span class="text-stone-500">· hidden {formatDateTime(h.hidden_at)}</span></span
						>
						<button class={small} disabled={busy} onclick={() => unhide(h)}>Suggest again</button>
					</li>
				{:else}
					<li class="px-3 py-2 text-stone-500">None.</li>
				{/each}
			</ul>
		{/if}

		{#if data.suggestions.suggestions.length}
			<ul class="mt-3 divide-y divide-stone-200 text-sm">
				{#each data.suggestions.suggestions as s (`${s.from.id}-${s.into.id}`)}
					{@const at = `${s.from.id}-${s.into.id}`}
					{@const [label, why] = likeness[s.likeness]}
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
								<span class="text-stone-400" aria-label="into">→</span>
								{@render entry(s.into)}
							</span>
							<span class="flex shrink-0 gap-1.5">
								<button
									class={small}
									disabled={busy}
									onclick={() => preview(at, data.kind, s.from.id, s.into.id)}>Merge…</button
								>
								<button
									class={small}
									disabled={busy}
									title="Keep “{s.from.name}” and merge “{s.into.name}” into it instead"
									onclick={() => preview(at, data.kind, s.into.id, s.from.id)}>Other way…</button
								>
								<button
									class={small}
									disabled={busy}
									title="These are different ones; don't suggest them again"
									onclick={() => hide(data.kind, s.from.id, s.into.id)}>Hide</button
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
		<h2 class={heading}>Merge by hand</h2>
		<p class="text-sm text-stone-600">
			Any two {kindLabel[data.kind]}s, also ones the suggestions don't find.
		</p>
		<div class="mt-3 flex flex-wrap items-end gap-3">
			<CatalogPicker kind={data.kind} label="Merge (goes away)" bind:picked={manualFrom} />
			<button
				class="{button} shrink-0"
				title="Swap"
				aria-label="Swap"
				disabled={busy}
				onclick={() => ([manualFrom, manualInto] = [manualInto, manualFrom])}>⇄</button
			>
			<CatalogPicker kind={data.kind} label="Into (stays)" bind:picked={manualInto} />
			<button
				class="{button} shrink-0"
				disabled={busy || !manualFrom || !manualInto}
				onclick={() => preview('manual', data.kind, manualFrom!.id, manualInto!.id)}>Preview</button
			>
		</div>
		{#if pending?.at === 'manual'}{@render previewBox(pending.preview)}{/if}
	</section>

	<section class="mt-10">
		<h2 class={heading}>Log</h2>
		<p class="text-sm text-stone-600">
			All merges, from here, the command line and <code>merge editions</code>, the latest first.
			Undoing one puts back what it changed; later merges of the same entries have to be undone
			first.
		</p>
		{#if log.length === 0}
			<p class="mt-3 text-sm text-stone-500">No merges yet.</p>
		{:else}
			<ul class="mt-3 divide-y divide-stone-200 text-sm">
				{#each log as op (op.id)}
					<li class="py-2">
						<div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
							<span class="w-12 shrink-0 text-stone-500 tabular-nums">#{op.id}</span>
							<span class="min-w-56 flex-1 {op.undone_at ? 'text-stone-500' : ''}">
								<span class="text-stone-500">{kindLabel[op.kind]}</span>
								<span class={op.undone_at ? 'line-through' : 'font-medium'}>“{op.from[1]}”</span>
								<span class="text-stone-500">#{op.from[0]}</span>
								<span class="text-stone-400" aria-label="into">→</span>
								<span class={op.undone_at ? 'line-through' : 'font-medium'}>“{op.into[1]}”</span>
								<span class="text-stone-500">#{op.into[0]}</span>
							</span>
							<span class="shrink-0 text-stone-500 tabular-nums"
								>{formatDateTime(op.merged_at)}</span
							>
							<span class="w-28 shrink-0 text-right">
								{#if op.undone_at}
									<span class="text-xs text-stone-500" title={formatDateTime(op.undone_at)}
										>undone</span
									>
								{:else}
									<button class={small} disabled={busy} onclick={() => previewUndo(op.id)}
										>Undo…</button
									>
								{/if}
							</span>
						</div>
						{#if undoing?.merge.id === op.id}
							<div class="mt-2 rounded border border-yellow-300 bg-yellow-50 p-3 text-sm">
								<p>
									Would undo merge {op.id}: {kindLabel[op.kind]} “{op.from[1]}” stands on its own
									again, with {count(undoing.restored, 'change', 'changes')} put back.
									{#if undoing.kept}
										{count(undoing.kept, 'row stays', 'rows stay')} as it is now, as it changed again
										since.
									{/if}
								</p>
								<div class="mt-3 flex gap-2">
									<button
										class="{button} border-yellow-500 font-medium"
										disabled={busy}
										onclick={() => undo(op.id)}>Undo merge</button
									>
									<button class={button} disabled={busy} onclick={() => (undoing = null)}
										>Cancel</button
									>
								</div>
							</div>
						{/if}
					</li>
				{/each}
			</ul>
			{#if next}
				<button class="{button} mt-4" disabled={busy} onclick={showMore}>Show more</button>
			{/if}
		{/if}
	</section>
</main>
