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

// TODO: Require this is called using a Signals root.
// TODO: Error on set if the root is closed, preventing potential errors.
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

export interface Memo<T> {
	get(): T;
	peek(): T;
	subscribe(fn: (value: T) => void): Dispose;
	memo<U>(fn: (value: T) => U): Memo<U>;
}

export type MemoOptions<T> = SignalOptions<T> & { deepEquals?: boolean };

// Unlike solid-js, this doesn't support passing an initial value.
// It calls the function immediately instead.
export function memo<T>(fn: (prev?: T) => T, options?: MemoOptions<T>): Memo<T> {
	if (options?.deepEquals) {
		options.equals = dequal;
	}

	const sig = signal(fn(undefined), options);
	effect(() => {
		const next = fn(sig.peek());
		sig.set(next);
	});

	return sig;
}

export function effect(fn: () => void) {
	createEffect(() => {
		const result = fn();
		if (typeof result === "function") {
			// Feels bad that Typescript can't enforce this.
			console.warn("effect must return void; use cleanup() instead");
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

	effect(fn: () => void): void {
		const res = runWithOwner(this.#owner, () => {
			effect(fn);
			return true;
		});
		if (!res) {
			throw new Error("root is closed");
		}
	}

	memo<T>(fn: (prev?: T) => T, options?: MemoOptions<T>): Memo<T> {
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
