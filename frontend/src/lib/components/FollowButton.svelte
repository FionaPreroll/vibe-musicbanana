<script lang="ts">
	import { errorMessage, profileApi, sendJson, type FollowInfo } from '#lib/api.ts';
	import { m } from '#lib/i18n/index.ts';

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
			!confirm(m.follow.confirmStop(info.username))
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
			{info.visibility === 'followers' ? m.follow.ask : m.follow.follow}
		</button>
	{:else if current === 'requested'}
		<span class="text-stone-600">{m.follow.asked(info.username)}</span>
		<button class={button} disabled={busy} onclick={() => act('DELETE')}>{m.follow.takeBack}</button
		>
	{:else}
		<span class="text-stone-600">{m.follow.following}</span>
		<button class={button} disabled={busy} onclick={() => act('DELETE')}>{m.follow.unfollow}</button
		>
	{/if}
	{#if error}<span class="text-red-700" role="alert">{error}</span>{/if}
</span>
