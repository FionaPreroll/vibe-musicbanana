<script lang="ts">
	// The footer easter egg of musicbanana2, unfolded by the banana in the header.
	import { slide } from 'svelte/transition';
	import { timeSince } from '#lib/format.ts';
	import { m } from '#lib/i18n/index.ts';

	let { open = $bindable(false) } = $props();

	const birthdays = {
		musicbanana: new Date('2007-08-12T14:28:00Z'),
		python: new Date('2010-03-27T17:05:00Z'),
		symfony: new Date('2020-08-24T17:54:46+02:00'),
		// The first commit of this repository.
		rust: new Date('2026-10-03T12:20:16Z')
	};

	let now = $state(new Date());
	$effect(() => {
		if (!open) return;
		now = new Date();
		const timer = setInterval(() => (now = new Date()), 60_000);
		return () => clearInterval(timer);
	});

	const since = (key: keyof typeof birthdays) => timeSince(birthdays[key], now);
	const title = (key: keyof typeof birthdays) => m.banana.sinceTitle(birthdays[key].toISOString());
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (open = false)} />

{#if open}
	<div id="banana" class="bg-black text-banana" transition:slide={{ duration: 250 }}>
		<!-- prettier-ignore -->
		<pre class="mx-auto max-w-6xl overflow-x-auto px-4 py-4 font-mono text-xs leading-tight sm:px-8 sm:text-sm">
 _
//\
V  \
 \  \_
  \,'.`-.
   |\ `. `.
   ( \  `. `-.                        _,.-:\
    \ \   `.  `-._             __..--' ,-';/
     \ `.   `-.   `-..___..---'   _.--' ,'/
      `. `.    `-._        __..--'    ,' /
        `. `-_     ``--..''       _.-' ,'
          `-_ `-.___        __,--'   ,'
             `-.__  `----"""    __.-'
                  `--..____..--'

<span title={title('musicbanana')}>{m.banana.lives(since('musicbanana'))}</span>
<span title={title('python')}>{m.banana.python(since('python'))}</span>
<span title={title('symfony')}>{m.banana.elephpant(since('symfony'))}</span>
<span title={title('rust')}>{m.banana.rustacean(since('rust'))}</span></pre>
	</div>
{/if}
