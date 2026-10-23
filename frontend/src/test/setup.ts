import '@testing-library/jest-dom/vitest';

// Transitions end at once. happy-dom's own animations reject a promise nobody
// waits for when Svelte cancels them, which Vitest reports as an error.
Element.prototype.animate = function () {
	const animation = {
		playState: 'finished',
		effect: null,
		currentTime: 0,
		cancel() {},
		finish() {},
		set onfinish(done: () => void) {
			queueMicrotask(done);
		}
	};
	return animation as unknown as Animation;
};
