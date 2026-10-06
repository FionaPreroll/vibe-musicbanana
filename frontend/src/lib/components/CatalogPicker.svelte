<script lang="ts">
	import { errorMessage, getJson, type CatalogEntry, type CatalogKind } from '#lib/api.ts';
	import { listenCount } from '#lib/format.ts';

	// Finds an entry of the catalog by name (or #id) for a merge by hand.
	let {
		kind,
		label,
		picked = $bindable(null)
	}: { kind: CatalogKind; label: string; picked: CatalogEntry | null } = $props();

	let query = $state('');
	let hits: CatalogEntry[] = $state([]);
	let error = $state('');
	let open = $state(false);

	// Asks once typing pauses; a newer query wins over an older answer.
	let asked = 0;
	$effect(() => {
		const q = query.trim();
		const k = kind;
		const n = ++asked;
		if (!q) {
			hits = [];
			return;
		}
		const timer = setTimeout(async () => {
			try {
				const found = await getJson<CatalogEntry[]>(fetch, `/api/admin/catalog/${k}`, { q });
				if (n === asked) {
					hits = found;
					error = '';
				}
			} catch (e) {
				if (n === asked) error = errorMessage(e);
			}
		}, 250);
		return () => clearTimeout(timer);
	});

	function pick(entry: CatalogEntry) {
		picked = entry;
		query = '';
		open = false;
	}
</script>

<div class="flex min-w-56 flex-1 flex-col gap-1 text-sm">
	<span>{label}</span>
	{#if picked}
		<div class="flex items-baseline gap-2 rounded border border-stone-300 bg-white px-2 py-1.5">
			<span class="min-w-0 flex-1 truncate">
				<span class="font-medium">{picked.name}</span>
				{#if picked.artist}<span class="text-stone-500">· {picked.artist}</span>{/if}
				<span class="text-stone-500">· #{picked.id} · {listenCount(picked.listens)}</span>
			</span>
			<button
				class="text-stone-500 hover:text-stone-900"
				aria-label="Pick another"
				onclick={() => (picked = null)}>✕</button
			>
		</div>
	{:else}
		<div class="relative">
			<input
				class="w-full rounded border border-stone-300 bg-white px-2 py-1.5"
				type="search"
				placeholder="Name or #id"
				aria-label={label}
				bind:value={query}
				onfocus={() => (open = true)}
				onblur={() => setTimeout(() => (open = false), 150)}
			/>
			{#if open && (hits.length || error)}
				<ul
					class="absolute z-10 mt-1 max-h-72 w-full overflow-y-auto rounded border border-stone-300 bg-white shadow"
				>
					{#if error}<li class="px-2 py-1.5 text-red-700">{error}</li>{/if}
					{#each hits as hit (hit.id)}
						<li>
							<button
								class="w-full px-2 py-1.5 text-left hover:bg-yellow-50"
								onmousedown={(e) => e.preventDefault()}
								onclick={() => pick(hit)}
							>
								<span class="font-medium">{hit.name}</span>
								{#if hit.artist}<span class="text-stone-500">· {hit.artist}</span>{/if}
								<span class="text-stone-500">· #{hit.id} · {listenCount(hit.listens)}</span>
							</button>
						</li>
					{/each}
				</ul>
			{/if}
		</div>
	{/if}
</div>
