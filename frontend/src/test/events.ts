import { vi } from 'vitest';

/** A stand-in for EventSource that tests can send events through. */
export class FakeEventSource {
	static all: FakeEventSource[] = [];
	listeners = new Map<string, (() => void)[]>();
	onopen: (() => void) | null = null;
	closed = false;

	constructor(public url: string) {
		FakeEventSource.all.push(this);
	}

	addEventListener(type: string, listener: () => void) {
		this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
	}

	/** Sends an event of `type`, as the server would. */
	emit(type: string) {
		this.listeners.get(type)?.forEach((f) => f());
	}

	close() {
		this.closed = true;
	}

	/** The connections made since the last call. */
	static install() {
		FakeEventSource.all = [];
		vi.stubGlobal('EventSource', FakeEventSource);
	}
}
