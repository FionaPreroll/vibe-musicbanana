import { vi } from 'vitest';

/**
 * Gives every element `width` pixels, and tells `bind:clientWidth` about it:
 * happy-dom lays nothing out and never calls a ResizeObserver.
 */
export function layOut(width: number) {
	vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(width);
	vi.stubGlobal(
		'ResizeObserver',
		class {
			constructor(private callback: ResizeObserverCallback) {}
			observe(target: Element) {
				this.callback(
					[{ target, contentRect: { width } } as unknown as ResizeObserverEntry],
					this as unknown as ResizeObserver
				);
			}
			unobserve() {}
			disconnect() {}
		}
	);
}
