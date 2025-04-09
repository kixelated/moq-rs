export type FanoutNext<T> = [T | undefined, Promise<FanoutNext<T>> | undefined];

// A queue that allows 0..N readers with no backpressure.
// Each reader gets a copy of the value.
export class FanoutWriter<T> {
	#resolve?: (v: FanoutNext<T>) => void;
	#reject?: (reason?: unknown) => void;

	constructor() {
		const promise = new Promise<FanoutNext<T>>((resolve, reject) => {
			this.#resolve = resolve;
			this.#reject = reject;
		});

		this.#latest = [undefined, promise];
	}

	set(v: T) {
		const resolve = this.#resolve;
		if (!resolve) {
			throw new Error("closed");
		}

		const promise = new Promise<FanoutNext<T>>((resolve, reject) => {
			this.#resolve = resolve;
			this.#reject = reject;
		});
		this.#latest = [v, promise];

		resolve(this.#latest);
	}

	close() {
		this.#latest = [undefined, undefined];
		this.#resolve?.(this.#latest);

		this.#resolve = undefined;
		this.#reject = undefined;
	}

	abort(reason?: unknown) {
		this.#latest = [undefined, undefined];
		this.#reject?.(reason);

		this.#resolve = undefined;
		this.#reject = undefined;
	}

	reader(): FanoutReader<T> {
		return new FanoutReader(this);
	}
}

export class FanoutReader<T> {
	readonly #fanout: FanoutWriter<T>;

	constructor(fanout: FanoutWriter<T>) {
		this.#fanout = fanout;
	}
}
