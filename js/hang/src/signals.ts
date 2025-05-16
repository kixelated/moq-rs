/// <reference types="vite/client" />

// A wrapper around solid-js signals to provide a more ergonomic API.

import {
	Owner,
	SignalOptions,
	createEffect,
	createRoot,
	createSignal,
	getOwner,
	onCleanup,
	runWithOwner,
	untrack,
} from "solid-js";

export { batch } from "solid-js";

export interface Signal<T> extends Derived<T> {
	set(value: T | ((prev: T) => T)): void;
	derived<U>(fn: (value: T) => U): Derived<U>;
	readonly(): Derived<T>;
}

export function signal<T>(initial: T, options?: SignalOptions<T>): Signal<T> {
	const [get, set] = createSignal(initial, options);
	return {
		get,
		set,
		peek: () => untrack(get),
		subscribe(fn) {
			const temp = new Signals();
			temp.effect(() => {
				fn(get());
			});
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
					const temp = new Signals();
					temp.effect(() => {
						fn(get());
					});
					return temp.close.bind(temp);
				},
			};
		},
	};
}

export function cleanup(fn: () => void) {
	onCleanup(fn);
}

export type Dispose = () => void;

// biome-ignore lint/suspicious/noConfusingVoidType: it's required to make the callback optional
export type MaybeDispose = (() => void) | void;

export interface Derived<T> {
	get(): T;
	peek(): T;
	subscribe(fn: (value: T) => void): Dispose;
}

export function derived<T>(fn: () => T, options?: SignalOptions<T>): Derived<T> {
	const sig = signal(fn(), options);
	effect(() => {
		sig.set(fn());
	});

	return sig;
}

export function effect(fn: () => MaybeDispose) {
	createEffect(() => {
		const res = fn();
		if (res) {
			onCleanup(res);
		}
	});
}

export class Signals {
	#id = Symbol();
	#dispose: Dispose;
	#owner: Owner;

	static dev = import.meta.env?.MODE !== "production";

	// Sanity check to make sure roots are being disposed on dev.
	static #finalizer = new FinalizationRegistry<string>((debugInfo) => {
		console.warn(`Root was garbage collected without being closed:\n${debugInfo}`);
	});

	constructor() {
		if (Signals.dev) {
			const debug = new Error("created here:").stack ?? "No stack";
			Signals.#finalizer.register(this.#id, debug, this.#id);
		}

		[this.#dispose, this.#owner] = createRoot((dispose) => {
			const owner = getOwner();
			if (!owner) throw new Error("no owner");

			return [dispose, owner];
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

	derived<T>(fn: () => T, options?: SignalOptions<T>): Derived<T> {
		const res = runWithOwner(this.#owner, () => derived(fn, options));
		if (!res) {
			throw new Error("derived called after root was closed");
		}

		return res;
	}

	cleanup(fn: () => void): void {
		const ok = runWithOwner(this.#owner, () => {
			onCleanup(fn);
			return true;
		});

		if (!ok) {
			fn();
		}
	}

	close(): void {
		this.#dispose();
		if (Signals.dev) {
			Signals.#finalizer.unregister(this.#id);
		}
	}
}

export function signals(): Signals {
	return new Signals();
}
