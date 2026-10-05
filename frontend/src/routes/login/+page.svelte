<script lang="ts">
	import { goto, invalidateAll } from '$app/navigation';
	import { page } from '$app/state';
	import { errorMessage, profilePath, sendJson } from '#lib/api.ts';

	let login = $state('');
	let password = $state('');
	let error = $state('');
	let busy = $state(false);

	// Only addresses on this site, so a link can't send anybody elsewhere after the login.
	function nextPath(username: string) {
		const next = page.url.searchParams.get('next');
		return next?.startsWith('/') && !next.startsWith('//') ? next : profilePath(username);
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = '';
		try {
			const { username } = await sendJson<{ username: string }>(fetch, 'POST', '/api/session', {
				login,
				password
			});
			await invalidateAll();
			await goto(nextPath(username));
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head>
	<title>Log in · musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-sm px-4 pb-16 sm:px-8">
	<h1 class="mt-10 text-2xl font-bold">Log in</h1>
	<form class="mt-6 flex flex-col gap-4" onsubmit={submit}>
		<label class="flex flex-col gap-1 text-sm">
			<span class="font-medium">User name or email</span>
			<input
				class="rounded border border-stone-300 px-3 py-2 text-base"
				autocomplete="username"
				required
				bind:value={login}
			/>
		</label>
		<label class="flex flex-col gap-1 text-sm">
			<span class="font-medium">Password</span>
			<input
				class="rounded border border-stone-300 px-3 py-2 text-base"
				type="password"
				autocomplete="current-password"
				required
				bind:value={password}
			/>
		</label>
		{#if error}<p class="text-sm text-red-700" role="alert">{error}</p>{/if}
		<button
			class="rounded bg-stone-900 px-4 py-2 font-medium text-white hover:bg-stone-700 disabled:opacity-50"
			disabled={busy}
		>
			{busy ? 'Logging in…' : 'Log in'}
		</button>
	</form>
	<p class="mt-6 text-sm text-stone-500">
		The password of the old musicbanana still works. A new account comes from
		<code class="text-stone-700">musicbanana account create</code> on the server.
	</p>
</main>
