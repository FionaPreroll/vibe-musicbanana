<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import {
		errorMessage,
		profilePath,
		sendJson,
		type Connection,
		type Follower,
		type Visibility
	} from '#lib/api.ts';
	import { connectionState, importing, refetchState } from '#lib/connections.ts';
	import { formatDateTime, listenCount } from '#lib/format.ts';
	import { browserTimeZone, zoneNames } from '#lib/zone.svelte.ts';
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

	const refetch = (c: Connection) => {
		if (
			!confirm(
				`Fetch the whole history from YourSpotify into ${c.profile} again? Plays it has already are skipped.`
			)
		)
			return;
		change('connections', () => sendJson(fetch, 'POST', `/api/me/yourspotify/${c.id}/refetch`));
	};

	// While an import runs or is asked for, ask again now and then.
	$effect(() => {
		if (!data.connections.some((c) => !c.started_at || importing(c) || c.refetch_requested_at))
			return;
		const timer = setInterval(() => invalidateAll(), 10_000);
		return () => clearInterval(timer);
	});

	// Followers

	const answer = (f: Follower, method: 'PUT' | 'DELETE') =>
		change('followers', () =>
			sendJson(
				fetch,
				method,
				`/api/me/followers/${encodeURIComponent(f.profile)}/${encodeURIComponent(f.username)}`
			)
		);

	function removeFollower(f: Follower) {
		const what = f.state === 'requested' ? `Say no to ${f.username}?` : `Remove ${f.username}?`;
		if (confirm(what)) answer(f, 'DELETE');
	}

	// Time zone and first weekday

	const zoneName = (zone: string) => zone.replaceAll('_', ' ');
	// The browser's list, with the saved zone in case this browser lacks it.
	const knownZones = zoneNames();
	const zones = $derived(
		me.time_zone && !knownZones.includes(me.time_zone) ? [...knownZones, me.time_zone] : knownZones
	);
	// Monday 2024-01-01 and the days after it name the weekdays.
	const weekdayName = new Intl.DateTimeFormat(undefined, { weekday: 'long' });
	const weekStarts = [1, 7, 6].map((n) => ({
		value: n,
		label: weekdayName.format(new Date(2024, 0, n))
	}));

	let time: { zone: string; weekStart: number } | null = $state(null);
	const shownTime = $derived(time ?? { zone: me.time_zone ?? '', weekStart: me.week_start });
	let timeSaved = $state(false);

	async function changeTime(event: SubmitEvent) {
		event.preventDefault();
		const body = { time_zone: shownTime.zone || null, week_start: shownTime.weekStart };
		timeSaved = false;
		if (await change('time', () => sendJson(fetch, 'PUT', '/api/me/time', body))) {
			time = null;
			timeSaved = true;
		}
	}

	// Streaks

	const setStreaks = (show: boolean) =>
		change('streaks', () => sendJson(fetch, 'PUT', '/api/me/streaks', { show_streaks: show }));

	// User name

	let username: string | null = $state(null);
	let renamedFrom: string | null = $state(null);

	async function changeUsername(event: SubmitEvent) {
		event.preventDefault();
		const old = me.username;
		if (await change('username', () => sendJson(fetch, 'PUT', '/api/me/username', { username }))) {
			renamedFrom = old;
			username = null;
		}
	}

	// Email address

	let email = $state({ address: null as string | null, password: '' });
	let emailChanged = $state(false);

	async function changeEmail(event: SubmitEvent) {
		event.preventDefault();
		emailChanged = false;
		const body = { email: email.address, password: email.password };
		if (await change('email', () => sendJson(fetch, 'PUT', '/api/me/email', body))) {
			email = { address: null, password: '' };
			emailChanged = true;
		}
	}

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
			{#each data.profiles as profile (profile.slug)}
				<li class="py-3">
					<div class="flex flex-wrap items-center gap-x-4 gap-y-2">
						<a class="font-medium hover:underline" href={profilePath(me.username, profile.slug)}
							>{profilePath(me.username, profile.slug)}</a
						>
						<span class="text-sm text-stone-500 tabular-nums">{listenCount(profile.listens)}</span>
						<a
							class="text-sm text-stone-600 underline hover:text-stone-900"
							href="/settings/history/{encodeURIComponent(profile.slug)}">Edit listening history</a
						>
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
			<a class="underline" href="https://github.com/Yooooomi/your_spotify">YourSpotify</a> keeps the
			history of a Spotify account. Connected to a profile, musicbanana imports its whole history,
			then the new plays every 15 minutes. One Spotify account per profile. When YourSpotify has
			imported an older Spotify export since, <em>Fetch all again</em> brings its plays; those already
			here are skipped.
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
						<span class="ml-auto flex gap-2">
							<button
								class={button}
								disabled={!!c.refetch_requested_at}
								title="Fetch the whole history again, for plays YourSpotify has added from before"
								onclick={() => refetch(c)}>Fetch all again</button
							>
							<button class={button} onclick={() => disconnect(c)}>Remove</button>
						</span>
						<p class="w-full {c.error ? 'text-red-700' : 'text-stone-500'}">{connectionState(c)}</p>
						{#if refetchState(c)}
							<p class="w-full text-stone-500">{refetchState(c)}</p>
						{/if}
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
		<h2 class={heading}>Followers</h2>
		<p class="mb-3 text-sm text-stone-600">
			Anybody logged in can follow a public profile. A profile for followers shows itself to those
			you said yes to; removing someone hides it from them again.
		</p>
		{#if data.followers.length === 0}
			<p class="text-sm text-stone-500">Nobody follows your profiles yet.</p>
		{:else}
			<ul class="divide-y divide-stone-200 text-sm">
				{#each data.followers as f (`${f.profile}/${f.username}`)}
					<li class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 py-2">
						<span>
							<a class="font-medium hover:underline" href={profilePath(f.username)}>{f.username}</a>
							<span class="text-stone-500">
								{f.state === 'requested' ? 'asks to follow' : 'follows'}
								<a class="hover:underline" href={profilePath(me.username, f.profile)}
									>{profilePath(me.username, f.profile)}</a
								>
								{f.state === 'requested' ? 'since' : 'as of'}
								{formatDateTime(f.since)}
							</span>
						</span>
						<span class="flex gap-2">
							{#if f.state === 'requested'}
								<button class={button} onclick={() => answer(f, 'PUT')}>Say yes</button>
								<button class={button} onclick={() => removeFollower(f)}>Say no</button>
							{:else}
								<button class={button} onclick={() => removeFollower(f)}>Remove</button>
							{/if}
						</span>
					</li>
				{/each}
			</ul>
		{/if}
		{@render error('followers')}
	</section>

	<section class="mt-12">
		<h2 class={heading}>Time</h2>
		<p class="mb-3 text-sm text-stone-600">
			Days, weeks, years and the listening clock follow your time zone, on your profiles and on
			everybody else's. Without one, they follow this browser's.
		</p>
		<form class="flex max-w-sm flex-col gap-3" onsubmit={changeTime}>
			<label class="flex flex-col gap-1 text-sm">
				<span>Time zone</span>
				<select
					class={select}
					value={shownTime.zone}
					onchange={(e) => (time = { ...shownTime, zone: e.currentTarget.value })}
				>
					<option value="">This browser's ({zoneName(browserTimeZone)})</option>
					{#each zones as zone (zone)}
						<option value={zone}>{zoneName(zone)}</option>
					{/each}
				</select>
			</label>
			<label class="flex flex-col gap-1 text-sm">
				<span>Weeks start on</span>
				<select
					class={select}
					value={shownTime.weekStart}
					onchange={(e) => (time = { ...shownTime, weekStart: Number(e.currentTarget.value) })}
				>
					{#each weekStarts as day (day.value)}
						<option value={day.value}>{day.label}</option>
					{/each}
				</select>
			</label>
			{@render error('time')}
			{#if timeSaved}
				<p class="text-sm text-green-800" role="status">Saved.</p>
			{/if}
			<button
				class="{button} self-start"
				disabled={time === null ||
					(time.zone === (me.time_zone ?? '') && time.weekStart === me.week_start)}>Save</button
			>
		</form>
	</section>

	<section class="mt-12">
		<h2 class={heading}>Streaks</h2>
		<label class="flex items-start gap-2 text-sm">
			<input
				class="mt-0.5"
				type="checkbox"
				checked={me.show_streaks}
				onchange={(e) => setStreaks(e.currentTarget.checked)}
			/>
			<span>
				Show streaks, the days in a row with listens, under "This week" on profile pages. Turned
				off, you don't see them on any profile, and nobody sees those of your profiles.
			</span>
		</label>
		{@render error('streaks')}
	</section>

	<section class="mt-12">
		<h2 class={heading}>User name</h2>
		<p class="mb-3 text-sm text-stone-600">
			It is in the address of your profiles and logs you in, as does your email address. Links with
			an old name lead to the new one, and nobody else can take it. Scrobble tokens and YourSpotify
			connections stay as they are.
		</p>
		<form class="flex max-w-sm gap-2" onsubmit={changeUsername}>
			<input
				class="{input} min-w-0 flex-1"
				aria-label="User name"
				autocomplete="username"
				required
				maxlength="32"
				value={username ?? me.username}
				oninput={(e) => (username = e.currentTarget.value)}
			/>
			<button class={button} disabled={username === null || username === me.username}>Rename</button
			>
		</form>
		{@render error('username')}
		{#if renamedFrom}
			<p class="mt-2 text-sm text-green-800" role="status">
				Renamed. Links with {renamedFrom} lead to
				<a class="underline" href={profilePath(me.username)}>{profilePath(me.username)}</a>.
			</p>
		{/if}
	</section>

	<section class="mt-12">
		<h2 class={heading}>Email address</h2>
		<p class="mb-3 text-sm text-stone-600">
			It logs you in, as does your user name. musicbanana sends no mail, so it is not checked.
		</p>
		<form class="flex max-w-sm flex-col gap-3" onsubmit={changeEmail}>
			<label class="flex flex-col gap-1 text-sm">
				<span>New address</span>
				<input
					class={input}
					type="email"
					autocomplete="email"
					required
					maxlength="254"
					value={email.address ?? me.email}
					oninput={(e) => (email.address = e.currentTarget.value)}
				/>
			</label>
			<label class="flex flex-col gap-1 text-sm">
				<span>Your password</span>
				<input
					class={input}
					type="password"
					autocomplete="current-password"
					required
					bind:value={email.password}
				/>
			</label>
			{@render error('email')}
			{#if emailChanged}
				<p class="text-sm text-green-800" role="status">Changed to {me.email}.</p>
			{/if}
			<button
				class="{button} self-start"
				disabled={email.address === null || email.address === me.email}>Change address</button
			>
		</form>
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

	<section class="mt-12">
		<h2 class={heading}>Your data</h2>
		<p class="mb-3 text-sm text-stone-600">
			One JSON file with everything musicbanana keeps about your account: the account, your profiles
			with all their listens (in ListenBrainz's import format) and their trash, follows, scrobble
			tokens, YourSpotify connections and logins. Passwords and tokens stay out.
		</p>
		<a class="{button} inline-block" href="/api/me/export" download>Download your data</a>
	</section>

	<section class="mt-12">
		<h2 class={heading}>Deleting a profile or the account</h2>
		<p class="text-sm text-stone-600">
			Ask the admin of this server. A deleted profile takes its listens, scrobble tokens,
			YourSpotify connection and followers along; a deleted account all its profiles and logins.
			This can't be undone.
		</p>
	</section>
</main>
