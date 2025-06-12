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

import { dequal } from "dequal";

export interface Signal<T> extends Memo<T> {
	set(value: T | ((prev: T) => T)): void;
	readonly(): Memo<T>;
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
		memo(fn) {
			return memo(() => fn(get()));
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
				memo(fn) {
					return memo(() => fn(get()));
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

export interface Memo<T> {
	get(): T;
	peek(): T;
	subscribe(fn: (value: T) => void): Dispose;
	memo<U>(fn: (value: T) => U): Memo<U>;
}

export type MemoOptions<T> = SignalOptions<T> & { deepEquals?: boolean };

export function memo<T>(fn: () => T, options?: MemoOptions<T>): Memo<T> {
	if (options?.deepEquals) {
		options.equals = dequal;
	}

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
	#dispose: Dispose;
	#owner: Owner;

	// @ts-ignore
	static dev = import.meta.env?.MODE !== "production";

	// Sanity check to make sure roots are being disposed on dev.
	static #finalizer = new FinalizationRegistry<string>((debugInfo) => {
		console.warn(`Root was garbage collected without being closed:\n${debugInfo}`);
	});

	constructor(parent?: Owner) {
		if (Signals.dev) {
			const debug = new Error("created here:").stack ?? "No stack";
			Signals.#finalizer.register(this, debug, this);
		}

		[this.#dispose, this.#owner] = createRoot((dispose) => {
			const owner = getOwner();
			if (!owner) throw new Error("no owner");

			return [dispose, owner];
		}, parent);
	}

	effect(fn: () => MaybeDispose): void {
		const res = runWithOwner(this.#owner, () => {
			effect(fn);
			return true;
		});
		if (!res) {
			throw new Error("root is closed");
		}
	}

	memo<T>(fn: () => T, options?: MemoOptions<T>): Memo<T> {
		const res = runWithOwner(this.#owner, () => memo(fn, options));
		if (!res) {
			throw new Error("root is closed");
		}

		return res;
	}

	cleanup(fn: () => void): void {
		const ok = runWithOwner(this.#owner, () => {
			onCleanup(fn);
			return true;
		});

		if (!ok) {
			throw new Error("root is closed");
		}
	}

	close(): void {
		this.#dispose();

		if (Signals.dev) {
			Signals.#finalizer.unregister(this);
		}
	}

	nest(): Signals {
		return new Signals(this.#owner);
	}
}

export function signals(): Signals {
	return new Signals();
}
