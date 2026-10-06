<script lang="ts">
	import { formatDateTime, formatListenTime, timeAgo } from '#lib/format.ts';
	import { now } from '#lib/now.svelte.ts';

	// A time in a list grouped by day: "5 minutes ago" or "14:28", the full date and time on hover.
	// `ago` shows how long ago at any age, for times that aren't the day's.
	let {
		at,
		ago = false,
		class: className = ''
	}: { at: string; ago?: boolean; class?: string } = $props();

	const when = $derived(new Date(at));
</script>

<time class={className} datetime={at} title={formatDateTime(at)}
	>{ago ? timeAgo(when, now()) : formatListenTime(when, now())}</time
>
