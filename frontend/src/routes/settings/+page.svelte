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
	import { dates, formatDateTime, formatOrList, listenCount } from '#lib/format.ts';
	import Fill from '#lib/i18n/Fill.svelte';
	import { catalogs, languageNames, m } from '#lib/i18n/index.ts';
	import { browserLanguage, useLanguage } from '#lib/i18n/locale.svelte.ts';
	import { browserTimeZone, zoneNames } from '#lib/zone.svelte.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const me = $derived(data.me);

	const visibilities = $derived<{ value: Visibility; label: string }[]>([
		{ value: 'public', label: m.visibility.public },
		{ value: 'followers', label: m.visibility.followers },
		{ value: 'private', label: m.visibility.private }
	]);

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
		if (!confirm(m.settings.tokens.confirmRevoke(label))) return;
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
		if (!confirm(m.settings.yourspotify.confirmDisconnect(c.profile))) return;
		change('connections', () => sendJson(fetch, 'DELETE', `/api/me/yourspotify/${c.id}`));
	};

	const refetch = (c: Connection) => {
		if (!confirm(m.settings.yourspotify.confirmRefetch(c.profile))) return;
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
		const what =
			f.state === 'requested'
				? m.settings.followers.confirmNo(f.username)
				: m.settings.followers.confirmRemove(f.username);
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
	const weekdayName = dates({ weekday: 'long' });
	const weekStarts = $derived(
		[1, 7, 6].map((n) => ({ value: n, label: weekdayName().format(new Date(2024, 0, n)) }))
	);

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

	// Language: shown in the new one at once, and saved for the account.

	const languages = languageNames();
	const browserLanguageName = catalogs[browserLanguage()].languageName;

	async function setLanguage(language: string) {
		useLanguage(language || null);
		await change('language', () =>
			sendJson(fetch, 'PUT', '/api/me/language', { language: language || null })
		);
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
			errors.password = m.settings.password.differ;
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
	<title>{m.settings.title} · musicbanana</title>
</svelte:head>

<main class="mx-auto max-w-3xl px-4 pb-16 sm:px-8">
	<h1 class="mt-6 text-3xl font-bold">{m.settings.title}</h1>
	<p class="mt-1 text-stone-600">{me.username} · {me.email}</p>

	<section class="mt-10">
		<h2 class={heading}>{m.settings.profiles.heading}</h2>
		<p class="mb-3 text-sm text-stone-600">
			{m.settings.profiles.intro}
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
							href="/settings/history/{encodeURIComponent(profile.slug)}"
							>{m.settings.profiles.editHistory}</a
						>
						<select
							class="{select} ml-auto text-sm"
							aria-label={m.settings.profiles.whoSees(profile.name)}
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
							aria-label={m.settings.profiles.nameOf(profile.slug)}
							value={names[profile.slug] ?? profile.name}
							oninput={(e) => (names[profile.slug] = e.currentTarget.value)}
						/>
						<button
							class={button}
							disabled={names[profile.slug] === undefined || names[profile.slug] === profile.name}
							>{m.settings.profiles.rename}</button
						>
					</form>
					{@render error(`profile-${profile.slug}`)}
				</li>
			{/each}
		</ul>

		<form class="mt-4 rounded border border-stone-200 p-4" onsubmit={createProfile}>
			<h3 class="font-medium">{m.settings.profiles.new}</h3>
			<div class="mt-3 grid gap-3 sm:grid-cols-3">
				<label class="flex flex-col gap-1 text-sm">
					<span>{m.settings.profiles.address}</span>
					<input
						class={input}
						required
						maxlength="32"
						pattern="[a-z0-9][a-z0-9\-]*"
						title={m.settings.profiles.addressRule}
						placeholder="spotify"
						bind:value={newProfile.slug}
					/>
				</label>
				<label class="flex flex-col gap-1 text-sm">
					<span>{m.settings.profiles.name}</span>
					<input class={input} required placeholder="Spotify" bind:value={newProfile.name} />
				</label>
				<label class="flex flex-col gap-1 text-sm">
					<span>{m.settings.profiles.whoSeesIt}</span>
					<select class={select} bind:value={newProfile.visibility}>
						{#each visibilities as v (v.value)}
							<option value={v.value}>{v.label}</option>
						{/each}
					</select>
				</label>
			</div>
			<p class="mt-2 text-sm text-stone-500">
				{m.settings.profiles.addressMakes(`/u/${me.username}/${newProfile.slug || 'spotify'}`)}
			</p>
			{@render error('new-profile')}
			<button class="{button} mt-3">{m.settings.profiles.create}</button>
		</form>
	</section>

	<section class="mt-12">
		<h2 class={heading}>{m.settings.tokens.heading}</h2>
		<p class="mb-3 text-sm text-stone-600">
			{m.settings.tokens.intro}
		</p>

		{#if created}
			<div class="mb-4 rounded border border-yellow-400 bg-yellow-50 p-4" role="status">
				<p class="text-sm">
					<Fill text={m.settings.tokens.created}>
						{#snippet label()}<span class="font-medium">{created!.label}</span>{/snippet}
					</Fill>
				</p>
				<input
					class="{input} mt-2 w-full bg-white font-mono text-sm"
					readonly
					value={created.token}
					onfocus={(e) => e.currentTarget.select()}
				/>
				<button class="{button} mt-2" onclick={() => (created = null)}
					>{m.settings.tokens.done}</button
				>
			</div>
		{/if}

		{#if data.tokens.length === 0}
			<p class="text-sm text-stone-500">{m.settings.tokens.none}</p>
		{:else}
			<table class="w-full text-sm">
				<thead class="text-left text-stone-500">
					<tr>
						<th class="py-1 font-medium">{m.settings.tokens.label}</th>
						<th class="py-1 font-medium">{m.settings.tokens.profile}</th>
						<th class="hidden py-1 font-medium sm:table-cell">{m.settings.tokens.lastUsed}</th>
						<th></th>
					</tr>
				</thead>
				<tbody class="divide-y divide-stone-200">
					{#each data.tokens as token (token.id)}
						<tr>
							<td class="py-2">{token.label}</td>
							<td class="py-2">{token.profile}</td>
							<td class="hidden py-2 text-stone-500 tabular-nums sm:table-cell">
								{token.last_used_at ? formatDateTime(token.last_used_at) : m.settings.tokens.never}
							</td>
							<td class="py-2 text-right">
								<button class={button} onclick={() => revoke(token.id, token.label)}
									>{m.settings.tokens.revoke}</button
								>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
		{@render error('tokens')}

		<form class="mt-4 flex flex-wrap items-end gap-3" onsubmit={createToken}>
			<label class="flex flex-col gap-1 text-sm">
				<span>{m.settings.tokens.profile}</span>
				<select class={select} bind:value={newToken.profile}>
					{#each me.profiles as profile (profile.slug)}
						<option value={profile.slug}>{profile.name}</option>
					{/each}
				</select>
			</label>
			<label class="flex min-w-48 flex-1 flex-col gap-1 text-sm">
				<span>{m.settings.tokens.label}</span>
				<input class={input} required placeholder="Navidrome" bind:value={newToken.label} />
			</label>
			<button class={button}>{m.settings.tokens.create}</button>
		</form>
		{@render error('new-token')}
	</section>

	<section class="mt-12">
		<h2 class={heading}>{m.settings.yourspotify.heading}</h2>
		<p class="mb-3 text-sm text-stone-600">
			<Fill text={m.settings.yourspotify.intro}>
				{#snippet yourspotify()}<a class="underline" href="https://github.com/Yooooomi/your_spotify"
						>YourSpotify</a
					>{/snippet}
				{#snippet fetchAll()}<em>{m.settings.yourspotify.fetchAll}</em>{/snippet}
			</Fill>
		</p>
		{#if data.connections.length > 0}
			<ul class="divide-y divide-stone-200">
				{#each data.connections as c (c.id)}
					<li class="flex flex-wrap items-baseline gap-x-4 gap-y-1 py-2 text-sm">
						<a class="font-medium hover:underline" href={profilePath(me.username, c.profile)}
							>{c.profile}</a
						>
						<span class="min-w-0 truncate text-stone-500">{c.url}</span>
						<span class="text-stone-500 tabular-nums"
							>{m.settings.yourspotify.soFar(listenCount(c.imported))}</span
						>
						<span class="ml-auto flex gap-2">
							<button
								class={button}
								disabled={!!c.refetch_requested_at}
								title={m.settings.yourspotify.fetchAllTitle}
								onclick={() => refetch(c)}>{m.settings.yourspotify.fetchAll}</button
							>
							<button class={button} onclick={() => disconnect(c)}>{m.common.remove}</button>
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
				<Fill text={m.settings.yourspotify.cliOnly}>
					{#snippet variable()}<code class="text-stone-700">YOURSPOTIFY_ALLOWED_URLS</code
						>{/snippet}
				</Fill>
			</p>
		{:else}
			<form
				class="mt-4 grid gap-3 sm:grid-cols-[auto_1fr_1fr_auto] sm:items-end"
				onsubmit={connect}
			>
				<label class="flex flex-col gap-1 text-sm">
					<span>{m.settings.yourspotify.profile}</span>
					<select class={select} bind:value={newConnection.profile}>
						{#each me.profiles as profile (profile.slug)}
							<option value={profile.slug}>{profile.name}</option>
						{/each}
					</select>
				</label>
				<label class="flex flex-col gap-1 text-sm">
					<span>{m.settings.yourspotify.address}</span>
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
					<span>{m.settings.yourspotify.token}</span>
					<input
						class={input}
						required
						type="password"
						autocomplete="off"
						bind:value={newConnection.token}
					/>
				</label>
				<button class={button} disabled={connecting}
					>{connecting ? m.settings.yourspotify.checking : m.settings.yourspotify.connect}</button
				>
			</form>
			<p class="mt-2 text-sm text-stone-500">
				{m.settings.yourspotify.addressHelp(formatOrList(data.allowed))}
			</p>
		{/if}
		{@render error('connection')}
	</section>

	<section class="mt-12">
		<h2 class={heading}>{m.settings.followers.heading}</h2>
		<p class="mb-3 text-sm text-stone-600">
			{m.settings.followers.intro}
		</p>
		{#if data.followers.length === 0}
			<p class="text-sm text-stone-500">{m.settings.followers.none}</p>
		{:else}
			<ul class="divide-y divide-stone-200 text-sm">
				{#each data.followers as f (`${f.profile}/${f.username}`)}
					<li class="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 py-2">
						<span class="text-stone-500">
							<Fill
								text={f.state === 'requested'
									? m.settings.followers.asks(formatDateTime(f.since))
									: m.settings.followers.follows(formatDateTime(f.since))}
							>
								{#snippet user()}<a
										class="font-medium text-stone-900 hover:underline"
										href={profilePath(f.username)}>{f.username}</a
									>{/snippet}
								{#snippet profile()}<a
										class="hover:underline"
										href={profilePath(me.username, f.profile)}
										>{profilePath(me.username, f.profile)}</a
									>{/snippet}
							</Fill>
						</span>
						<span class="flex gap-2">
							{#if f.state === 'requested'}
								<button class={button} onclick={() => answer(f, 'PUT')}
									>{m.settings.followers.yes}</button
								>
								<button class={button} onclick={() => removeFollower(f)}
									>{m.settings.followers.no}</button
								>
							{:else}
								<button class={button} onclick={() => removeFollower(f)}>{m.common.remove}</button>
							{/if}
						</span>
					</li>
				{/each}
			</ul>
		{/if}
		{@render error('followers')}
	</section>

	<section class="mt-12">
		<h2 class={heading}>{m.settings.time.heading}</h2>
		<p class="mb-3 text-sm text-stone-600">
			{m.settings.time.intro}
		</p>
		<form class="flex max-w-sm flex-col gap-3" onsubmit={changeTime}>
			<label class="flex flex-col gap-1 text-sm">
				<span>{m.settings.time.zone}</span>
				<select
					class={select}
					value={shownTime.zone}
					onchange={(e) => (time = { ...shownTime, zone: e.currentTarget.value })}
				>
					<option value="">{m.settings.time.browserZone(zoneName(browserTimeZone))}</option>
					{#each zones as zone (zone)}
						<option value={zone}>{zoneName(zone)}</option>
					{/each}
				</select>
			</label>
			<label class="flex flex-col gap-1 text-sm">
				<span>{m.settings.time.weekStart}</span>
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
				<p class="text-sm text-green-800" role="status">{m.common.saved}</p>
			{/if}
			<button
				class="{button} self-start"
				disabled={time === null ||
					(time.zone === (me.time_zone ?? '') && time.weekStart === me.week_start)}
				>{m.common.save}</button
			>
		</form>
	</section>

	<section class="mt-12">
		<h2 class={heading}>{m.settings.language.heading}</h2>
		<p class="mb-3 text-sm text-stone-600">{m.settings.language.intro}</p>
		<select
			class="{select} max-w-sm"
			aria-label={m.settings.language.heading}
			value={me.language ?? ''}
			onchange={(e) => setLanguage(e.currentTarget.value)}
		>
			<option value="">{m.settings.language.browser(browserLanguageName)}</option>
			{#each languages as l (l.code)}
				<option value={l.code} lang={l.code}>{l.name}</option>
			{/each}
		</select>
		{@render error('language')}
	</section>

	<section class="mt-12">
		<h2 class={heading}>{m.settings.streaks.heading}</h2>
		<label class="flex items-start gap-2 text-sm">
			<input
				class="mt-0.5"
				type="checkbox"
				checked={me.show_streaks}
				onchange={(e) => setStreaks(e.currentTarget.checked)}
			/>
			<span>
				{m.settings.streaks.text}
			</span>
		</label>
		{@render error('streaks')}
	</section>

	<section class="mt-12">
		<h2 class={heading}>{m.settings.username.heading}</h2>
		<p class="mb-3 text-sm text-stone-600">
			{m.settings.username.intro}
		</p>
		<form class="flex max-w-sm gap-2" onsubmit={changeUsername}>
			<input
				class="{input} min-w-0 flex-1"
				aria-label={m.settings.username.label}
				autocomplete="username"
				required
				maxlength="32"
				value={username ?? me.username}
				oninput={(e) => (username = e.currentTarget.value)}
			/>
			<button class={button} disabled={username === null || username === me.username}
				>{m.settings.username.rename}</button
			>
		</form>
		{@render error('username')}
		{#if renamedFrom}
			<p class="mt-2 text-sm text-green-800" role="status">
				<Fill text={m.settings.username.renamed(renamedFrom)}>
					{#snippet link()}<a class="underline" href={profilePath(me.username)}
							>{profilePath(me.username)}</a
						>{/snippet}
				</Fill>
			</p>
		{/if}
	</section>

	<section class="mt-12">
		<h2 class={heading}>{m.settings.email.heading}</h2>
		<p class="mb-3 text-sm text-stone-600">
			{m.settings.email.intro}
		</p>
		<form class="flex max-w-sm flex-col gap-3" onsubmit={changeEmail}>
			<label class="flex flex-col gap-1 text-sm">
				<span>{m.settings.email.new}</span>
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
				<span>{m.settings.email.password}</span>
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
				<p class="text-sm text-green-800" role="status">{m.settings.email.changed(me.email)}</p>
			{/if}
			<button
				class="{button} self-start"
				disabled={email.address === null || email.address === me.email}
				>{m.settings.email.change}</button
			>
		</form>
	</section>

	<section class="mt-12">
		<h2 class={heading}>{m.settings.password.heading}</h2>
		<form class="flex max-w-sm flex-col gap-3" onsubmit={changePassword}>
			<label class="flex flex-col gap-1 text-sm">
				<span>{m.settings.password.current}</span>
				<input
					class={input}
					type="password"
					autocomplete="current-password"
					required
					bind:value={password.current}
				/>
			</label>
			<label class="flex flex-col gap-1 text-sm">
				<span>{m.settings.password.new}</span>
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
				<span>{m.settings.password.again}</span>
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
					{m.settings.password.changed}
				</p>
			{/if}
			<button class="{button} self-start">{m.settings.password.change}</button>
		</form>
	</section>

	<section class="mt-12">
		<h2 class={heading}>{m.settings.data.heading}</h2>
		<p class="mb-3 text-sm text-stone-600">
			{m.settings.data.intro}
		</p>
		<a class="{button} inline-block" href="/api/me/export" download>{m.settings.data.download}</a>
	</section>

	<section class="mt-12">
		<h2 class={heading}>{m.settings.delete.heading}</h2>
		<p class="text-sm text-stone-600">
			{m.settings.delete.text}
		</p>
	</section>
</main>
