<script lang="ts">
	// Light or dark. The page follows the system until someone picks the other theme
	// here; the browser remembers that pick. The script in app.html applies it before
	// the first paint.
	import { onMount } from 'svelte';

	let dark = $state(false);

	const systemDark = () => matchMedia('(prefers-color-scheme: dark)');

	function stored() {
		try {
			return localStorage.getItem('theme');
		} catch {
			return null;
		}
	}

	function apply(on: boolean) {
		dark = on;
		document.documentElement.dataset.theme = on ? 'dark' : 'light';
	}

	onMount(() => {
		dark = document.documentElement.dataset.theme === 'dark';
		const system = systemDark();
		const follow = () => {
			if (stored() === null) apply(system.matches);
		};
		system.addEventListener('change', follow);
		return () => system.removeEventListener('change', follow);
	});

	function toggle() {
		apply(!dark);
		// Picking what the system shows anyway means following the system again.
		try {
			if (dark === systemDark().matches) localStorage.removeItem('theme');
			else localStorage.setItem('theme', dark ? 'dark' : 'light');
		} catch {
			// Without storage the pick lasts until the page reloads.
		}
	}
</script>

<button
	type="button"
	class="cursor-pointer self-center text-stone-600 hover:text-stone-900"
	aria-pressed={dark}
	aria-label="Dark mode"
	title={dark ? 'Switch to light mode' : 'Switch to dark mode'}
	onclick={toggle}
>
	{#if dark}
		<svg
			class="size-4"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			aria-hidden="true"
		>
			<circle cx="12" cy="12" r="4" />
			<path
				d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41"
			/>
		</svg>
	{:else}
		<svg
			class="size-4"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linejoin="round"
			aria-hidden="true"
		>
			<path d="M20.5 14.5A8.5 8.5 0 0 1 9.5 3.5a8.5 8.5 0 1 0 11 11Z" />
		</svg>
	{/if}
</button>
