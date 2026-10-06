import { createSubscriber } from 'svelte/reactivity';

// Re-runs whatever reads `now()` every 30 seconds, so "5 minutes ago" and
// "Today" stay right while a page is open.
const subscribe = createSubscriber((update) => {
	const timer = setInterval(update, 30_000);
	return () => clearInterval(timer);
});

/** The current time, reactive in components. */
export function now() {
	subscribe();
	return new Date();
}
