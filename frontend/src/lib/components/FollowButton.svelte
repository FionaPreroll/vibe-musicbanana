<script lang="ts">
	import { errorMessage, profileApi, sendJson, type FollowInfo } from '#lib/api.ts';

	// Follow, ask to follow, take the request back or stop following.
	let { info, onchange }: { info: FollowInfo; onchange?: (info: FollowInfo) => void } = $props();

	let current = $derived(info.state);
	let busy = $state(false);
	let error = $state('');

	const button =
		'rounded border border-stone-300 bg-white px-3 py-1 text-sm hover:bg-stone-100 disabled:opacity-50';

	async function act(method: 'PUT' | 'DELETE') {
		if (
			method === 'DELETE' &&
			current === 'following' &&
			info.visibility === 'followers' &&
			!confirm(`Stop following? You won't see this profile until ${info.username} says yes again.`)
		) {
			return;
		}
		busy = true;
		error = '';
		try {
			const api = `${profileApi(info.username, info.slug)}/follow`;
			const answer = await sendJson<FollowInfo | null>(fetch, method, api);
			const next = answer ?? { ...info, state: 'none' as const };
			current = next.state;
			onchange?.(next);
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
</script>

<span class="inline-flex flex-wrap items-baseline gap-2 text-sm">
	{#if current === 'none'}
		<button class={button} disabled={busy} onclick={() => act('PUT')}>
			{info.visibility === 'followers' ? 'Ask to follow' : 'Follow'}
		</button>
	{:else if current === 'requested'}
		<span class="text-stone-600">Asked {info.username} to follow</span>
		<button class={button} disabled={busy} onclick={() => act('DELETE')}>Take back</button>
	{:else}
		<span class="text-stone-600">Following</span>
		<button class={button} disabled={busy} onclick={() => act('DELETE')}>Unfollow</button>
	{/if}
	{#if error}<span class="text-red-700" role="alert">{error}</span>{/if}
</span>
