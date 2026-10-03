<script lang="ts">
	type Health = { status: string; listens: number };

	const health: Promise<Health> = fetch('/api/health').then((res) => {
		if (!res.ok) throw new Error(`HTTP ${res.status}`);
		return res.json();
	});
</script>

<svelte:head>
	<title>musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-2xl p-8">
	<h1 class="text-4xl font-bold">musicbanana 🍌</h1>
	<p class="mt-2 text-gray-600">A free place for your listening history.</p>

	<p class="mt-8 text-sm">
		{#await health}
			Checking backend…
		{:then h}
			Backend {h.status}, {h.listens} listens stored.
		{:catch err}
			<span class="text-red-600">Backend unreachable ({err.message}).</span>
		{/await}
	</p>
</main>
