// A wrapper around solid-js signals to provide a more ergonomic API.

import { createEffect, createRoot, createSignal, untrack, getOwner, Owner, runWithOwner, onCleanup } from "solid-js";

declare global {
	interface ImportMeta {
		env?: {
			MODE: string;
		};
	}
}

export interface Signal<T> extends Derived<T> {
	set(value: T | ((prev: T) => T)): void;
	derived<U>(fn: (value: T) => U): Derived<U>;
	readonly(): Derived<T>;
}

export function signal<T>(initial: T): Signal<T> {
	const [get, set] = createSignal(initial);
	return {
		get,
		set,
		peek: () => untrack(get),
		subscribe(fn) {
			const temp = new Root();
			temp.effect(() => fn(get()));
			return temp.close.bind(temp);
		},
		derived(fn) {
			return derived(() => fn(get()));
		},
		readonly() {
			return {
				get: () => get(),
				peek: () => untrack(get),
				subscribe(fn) {
					const temp = new Root();
					temp.effect(() => fn(get()));
					return temp.close.bind(temp);
				},
			};
		},
	};
}

/*
export function root(fn: () => void): Dispose {
	return createRoot((dispose) => {
		effect(fn);
		return dispose;
	});
}
*/

export type Dispose = () => void;
// biome-ignore lint/suspicious/noConfusingVoidType: pls
export type MaybeDispose = (() => void) | void;

export interface Derived<T> {
	get(): T;
	peek(): T;
	subscribe(fn: (value: T) => void): Dispose;
}

// Not public so you can't call it without a root.
function derived<T>(fn: () => T): Derived<T> {
	const sig = signal(fn());
	effect(() => {
		sig.set(fn());
	});

	return sig;
}

// Not public so you can't call it without a root.
function effect(fn: () => MaybeDispose) {
	return createEffect(() => {
		const res = fn();
		if (res) {
			onCleanup(res);
		}
	});
}

export class Root {
	#id = Symbol();
	#dispose: Dispose;
	#owner: Owner;

	static dev = import.meta.env?.MODE !== "production";

	// Sanity check to make sure roots are being disposed on dev.
	static #finalizer = new FinalizationRegistry<string>((debugInfo) => {
		console.warn(`Root was garbage collected without being closed:\n${debugInfo}`);
	});

	constructor() {
		if (Root.dev) {
			const debug = new Error("created here:").stack ?? "No stack";
			Root.#finalizer.register(this.#id, debug, this.#id);
		}

		[this.#dispose, this.#owner] = createRoot((dispose) => {
			return [dispose, getOwner()!];
		});
	}

	effect(fn: () => MaybeDispose): void {
		const res = runWithOwner(this.#owner, () => {
			effect(fn);
			return true;
		});
		if (!res) {
			throw new Error("effect called after root was closed");
		}
	}

	derived<T>(fn: () => T): Derived<T> {
		const res = runWithOwner(this.#owner, () => derived(fn));
		if (!res) {
			throw new Error("derived called after root was closed");
		}

		return res;
	}

	close(): void {
		this.#dispose();
		if (Root.dev) {
			Root.#finalizer.unregister(this.#id);
		}
	}
}

export function root(): Root {
	return new Root();
}
