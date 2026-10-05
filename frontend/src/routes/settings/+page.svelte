<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import {
		errorMessage,
		profilePath,
		sendJson,
		type Connection,
		type Visibility
	} from '#lib/api.ts';
	import { formatDateTime, listenCount } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const me = $derived(data.me);

	const visibilities: { value: Visibility; label: string }[] = [
		{ value: 'public', label: 'Public' },
		{ value: 'followers', label: 'Followers only' },
		{ value: 'private', label: 'Private' }
	];

	// Each form keeps its own error, so it shows next to what failed.
	let errors: Record<string, string> = $state({});

	/** Runs a change, then reloads the page's data (and the header's) from the server. */
	async function change(form: string, action: () => Promise<unknown>) {
		errors[form] = '';
		try {
			await action();
			await invalidateAll();
			return true;
		} catch (e) {
			errors[form] = errorMessage(e);
			return false;
		}
	}

	// Profiles

	let names: Record<string, string> = $state({});

	const rename = (slug: string) =>
		change(`profile-${slug}`, () =>
			sendJson(fetch, 'PATCH', `/api/me/profiles/${encodeURIComponent(slug)}`, {
				name: names[slug]
			})
		).then((done) => done && delete names[slug]);

	const setVisibility = (slug: string, visibility: Visibility) =>
		change(`profile-${slug}`, () =>
			sendJson(fetch, 'PATCH', `/api/me/profiles/${encodeURIComponent(slug)}`, { visibility })
		);

	let newProfile = $state({ slug: '', name: '', visibility: 'public' as Visibility });

	async function createProfile(event: SubmitEvent) {
		event.preventDefault();
		if (
			await change('new-profile', () => sendJson(fetch, 'POST', '/api/me/profiles', newProfile))
		) {
			newProfile = { slug: '', name: '', visibility: 'public' };
		}
	}

	// Tokens

	let newToken = $state({ profile: 'default', label: '' });
	let created: { label: string; token: string } | null = $state(null);

	async function createToken(event: SubmitEvent) {
		event.preventDefault();
		const label = newToken.label;
		await change('new-token', async () => {
			const { token } = await sendJson<{ token: string }>(
				fetch,
				'POST',
				'/api/me/tokens',
				newToken
			);
			created = { label, token };
			newToken.label = '';
		});
	}

	const revoke = (id: number, label: string) => {
		if (!confirm(`Revoke the token "${label}"? Its client can't scrobble any more.`)) return;
		change('tokens', () => sendJson(fetch, 'DELETE', `/api/me/tokens/${id}`));
	};

	// YourSpotify

	// svelte-ignore state_referenced_locally
	let newConnection = $state({
		profile: 'default',
		url: data.allowed.length === 1 ? data.allowed[0] : '',
		token: ''
	});
	let connecting = $state(false);

	async function connect(event: SubmitEvent) {
		event.preventDefault();
		connecting = true;
		if (
			await change('connection', () =>
				sendJson(fetch, 'POST', '/api/me/yourspotify', newConnection)
			)
		) {
			newConnection = { profile: 'default', url: newConnection.url, token: '' };
		}
		connecting = false;
	}

	const disconnect = (c: Connection) => {
		if (!confirm(`Stop importing from YourSpotify into ${c.profile}? Its listens stay.`)) return;
		change('connections', () => sendJson(fetch, 'DELETE', `/api/me/yourspotify/${c.id}`));
	};

	function connectionState(c: Connection) {
		if (!c.started_at) return 'Starts within a minute.';
		if (!c.finished_at || c.finished_at < c.started_at) {
			return `Importing since ${formatDateTime(c.started_at)}…`;
		}
		if (c.error) return `Failed at ${formatDateTime(c.finished_at)}: ${c.error}`;
		return `Up to date as of ${formatDateTime(c.finished_at)}.`;
	}

	// While an import runs, ask again now and then.
	$effect(() => {
		if (!data.connections.some((c) => !c.finished_at || (c.started_at ?? '') > c.finished_at))
			return;
		const timer = setInterval(() => invalidateAll(), 10_000);
		return () => clearInterval(timer);
	});

	// Password

	let password = $state({ current: '', new: '', repeated: '' });
	let passwordChanged = $state(false);

	async function changePassword(event: SubmitEvent) {
		event.preventDefault();
		passwordChanged = false;
		if (password.new !== password.repeated) {
			errors.password = 'The new passwords differ.';
			return;
		}
		const { current, new: newPassword } = password;
		if (
			await change('password', () =>
				sendJson(fetch, 'PUT', '/api/me/password', { current, new: newPassword })
			)
		) {
			password = { current: '', new: '', repeated: '' };
			passwordChanged = true;
		}
	}

	const input = 'rounded border border-stone-300 px-2 py-1.5';
	const select = `${input} pr-8`;
	const button =
		'rounded border border-stone-300 px-3 py-1.5 text-sm hover:bg-stone-100 disabled:opacity-50';
	const heading = 'mb-3 text-sm font-semibold tracking-wide text-stone-500 uppercase';
</script>

{#snippet error(form: string)}
	{#if errors[form]}<p class="mt-2 text-sm text-red-700" role="alert">{errors[form]}</p>{/if}
{/snippet}

<svelte:head>
	<title>Settings · musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-3xl px-4 pb-16 sm:px-8">
	<h1 class="mt-6 text-3xl font-bold">Settings</h1>
	<p class="mt-1 text-stone-600">{me.username} · {me.email}</p>

	<section class="mt-10">
		<h2 class={heading}>Profiles</h2>
		<p class="mb-3 text-sm text-stone-600">
			Each profile has its own listens and charts, for example one per player or per person.
		</p>
		<ul class="divide-y divide-stone-200">
			{#each me.profiles as profile (profile.slug)}
				<li class="py-3">
					<div class="flex flex-wrap items-center gap-x-4 gap-y-2">
						<a class="font-medium hover:underline" href={profilePath(me.username, profile.slug)}
							>{profilePath(me.username, profile.slug)}</a
						>
						<span class="text-sm text-stone-500 tabular-nums">{listenCount(profile.listens)}</span>
						<select
							class="{select} ml-auto text-sm"
							aria-label="Who sees {profile.name}"
							value={profile.visibility}
							onchange={(e) => setVisibility(profile.slug, e.currentTarget.value as Visibility)}
						>
							{#each visibilities as v (v.value)}
								<option value={v.value}>{v.label}</option>
							{/each}
						</select>
					</div>
					<form
						class="mt-2 flex gap-2"
						onsubmit={(e) => {
							e.preventDefault();
							rename(profile.slug);
						}}
					>
						<input
							class="{input} min-w-0 flex-1"
							aria-label="Name of {profile.slug}"
							value={names[profile.slug] ?? profile.name}
							oninput={(e) => (names[profile.slug] = e.currentTarget.value)}
						/>
						<button
							class={button}
							disabled={names[profile.slug] === undefined || names[profile.slug] === profile.name}
							>Rename</button
						>
					</form>
					{@render error(`profile-${profile.slug}`)}
				</li>
			{/each}
		</ul>

		<form class="mt-4 rounded border border-stone-200 p-4" onsubmit={createProfile}>
			<h3 class="font-medium">New profile</h3>
			<div class="mt-3 grid gap-3 sm:grid-cols-3">
				<label class="flex flex-col gap-1 text-sm">
					<span>Address</span>
					<input
						class={input}
						required
						maxlength="32"
						pattern="[a-z0-9][a-z0-9\-]*"
						title="Lower-case letters, digits and dashes"
						placeholder="spotify"
						bind:value={newProfile.slug}
					/>
				</label>
				<label class="flex flex-col gap-1 text-sm">
					<span>Name</span>
					<input class={input} required placeholder="Spotify" bind:value={newProfile.name} />
				</label>
				<label class="flex flex-col gap-1 text-sm">
					<span>Who sees it</span>
					<select class={select} bind:value={newProfile.visibility}>
						{#each visibilities as v (v.value)}
							<option value={v.value}>{v.label}</option>
						{/each}
					</select>
				</label>
			</div>
			<p class="mt-2 text-sm text-stone-500">
				The address makes /u/{me.username}/{newProfile.slug || 'spotify'}.
			</p>
			{@render error('new-profile')}
			<button class="{button} mt-3">Create profile</button>
		</form>
	</section>

	<section class="mt-12">
		<h2 class={heading}>Scrobble tokens</h2>
		<p class="mb-3 text-sm text-stone-600">
			A player scrobbles with a token, which decides the profile its listens go to. In Navidrome it
			goes into the personal settings under "Scrobble to ListenBrainz".
		</p>

		{#if created}
			<div class="mb-4 rounded border border-yellow-400 bg-yellow-50 p-4" role="status">
				<p class="text-sm">
					The token for <span class="font-medium">{created.label}</span>. It is shown only now, so
					copy it into the player:
				</p>
				<input
					class="{input} mt-2 w-full bg-white font-mono text-sm"
					readonly
					value={created.token}
					onfocus={(e) => e.currentTarget.select()}
				/>
				<button class="{button} mt-2" onclick={() => (created = null)}>Done</button>
			</div>
		{/if}

		{#if data.tokens.length === 0}
			<p class="text-sm text-stone-500">No tokens yet.</p>
		{:else}
			<table class="w-full text-sm">
				<thead class="text-left text-stone-500">
					<tr>
						<th class="py-1 font-medium">Label</th>
						<th class="py-1 font-medium">Profile</th>
						<th class="hidden py-1 font-medium sm:table-cell">Last used</th>
						<th></th>
					</tr>
				</thead>
				<tbody class="divide-y divide-stone-200">
					{#each data.tokens as token (token.id)}
						<tr>
							<td class="py-2">{token.label}</td>
							<td class="py-2">{token.profile}</td>
							<td class="hidden py-2 text-stone-500 tabular-nums sm:table-cell">
								{token.last_used_at ? formatDateTime(token.last_used_at) : 'never'}
							</td>
							<td class="py-2 text-right">
								<button class={button} onclick={() => revoke(token.id, token.label)}>Revoke</button>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
		{@render error('tokens')}

		<form class="mt-4 flex flex-wrap items-end gap-3" onsubmit={createToken}>
			<label class="flex flex-col gap-1 text-sm">
				<span>Profile</span>
				<select class={select} bind:value={newToken.profile}>
					{#each me.profiles as profile (profile.slug)}
						<option value={profile.slug}>{profile.name}</option>
					{/each}
				</select>
			</label>
			<label class="flex min-w-48 flex-1 flex-col gap-1 text-sm">
				<span>Label</span>
				<input class={input} required placeholder="Navidrome" bind:value={newToken.label} />
			</label>
			<button class={button}>Create token</button>
		</form>
		{@render error('new-token')}
	</section>

	<section class="mt-12">
		<h2 class={heading}>Spotify via YourSpotify</h2>
		<p class="mb-3 text-sm text-stone-600">
			<a class="underline" href="https://github.com/Yooooomi/your_spotify">YourSpotify</a> keeps the history
			of a Spotify account. Connected to a profile, musicbanana imports its whole history, then the new
			plays every 15 minutes. One Spotify account per profile.
		</p>
		{#if data.connections.length > 0}
			<ul class="divide-y divide-stone-200">
				{#each data.connections as c (c.id)}
					<li class="flex flex-wrap items-baseline gap-x-4 gap-y-1 py-2 text-sm">
						<a class="font-medium hover:underline" href={profilePath(me.username, c.profile)}
							>{c.profile}</a
						>
						<span class="min-w-0 truncate text-stone-500">{c.url}</span>
						<span class="text-stone-500 tabular-nums">{listenCount(c.imported)} so far</span>
						<button class="{button} ml-auto" onclick={() => disconnect(c)}>Remove</button>
						<p class="w-full {c.error ? 'text-red-700' : 'text-stone-500'}">{connectionState(c)}</p>
					</li>
				{/each}
			</ul>
		{/if}
		{@render error('connections')}

		{#if data.allowed.length === 0}
			<p class="mt-4 text-sm text-stone-500">
				This server takes YourSpotify connections from its command line only. Its admin can allow
				addresses for this page with <code class="text-stone-700">YOURSPOTIFY_ALLOWED_URLS</code>.
			</p>
		{:else}
			<form
				class="mt-4 grid gap-3 sm:grid-cols-[auto_1fr_1fr_auto] sm:items-end"
				onsubmit={connect}
			>
				<label class="flex flex-col gap-1 text-sm">
					<span>Profile</span>
					<select class={select} bind:value={newConnection.profile}>
						{#each me.profiles as profile (profile.slug)}
							<option value={profile.slug}>{profile.name}</option>
						{/each}
					</select>
				</label>
				<label class="flex flex-col gap-1 text-sm">
					<span>YourSpotify API address</span>
					<input
						class={input}
						required
						type="url"
						list="yourspotify-allowed"
						placeholder={data.allowed[0]}
						bind:value={newConnection.url}
					/>
					<datalist id="yourspotify-allowed">
						{#each data.allowed as address (address)}<option value={address}></option>{/each}
					</datalist>
				</label>
				<label class="flex flex-col gap-1 text-sm">
					<span>Public token</span>
					<input
						class={input}
						required
						type="password"
						autocomplete="off"
						bind:value={newConnection.token}
					/>
				</label>
				<button class={button} disabled={connecting}>{connecting ? 'Checking…' : 'Connect'}</button>
			</form>
			<p class="mt-2 text-sm text-stone-500">
				The address is YourSpotify's API (its API_ENDPOINT, not the web interface), as the
				musicbanana server reaches it: {data.allowed.join(' or ')}, or below. The public token is in
				YourSpotify's settings.
			</p>
		{/if}
		{@render error('connection')}
	</section>

	<section class="mt-12">
		<h2 class={heading}>Password</h2>
		<form class="flex max-w-sm flex-col gap-3" onsubmit={changePassword}>
			<label class="flex flex-col gap-1 text-sm">
				<span>Current password</span>
				<input
					class={input}
					type="password"
					autocomplete="current-password"
					required
					bind:value={password.current}
				/>
			</label>
			<label class="flex flex-col gap-1 text-sm">
				<span>New password</span>
				<input
					class={input}
					type="password"
					autocomplete="new-password"
					required
					minlength="8"
					bind:value={password.new}
				/>
			</label>
			<label class="flex flex-col gap-1 text-sm">
				<span>New password again</span>
				<input
					class={input}
					type="password"
					autocomplete="new-password"
					required
					minlength="8"
					bind:value={password.repeated}
				/>
			</label>
			{@render error('password')}
			{#if passwordChanged}
				<p class="text-sm text-green-800" role="status">
					Changed. Other browsers logged in with this account are logged out.
				</p>
			{/if}
			<button class="{button} self-start">Change password</button>
		</form>
	</section>
</main>
