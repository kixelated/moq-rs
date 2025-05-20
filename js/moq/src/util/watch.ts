export class Deferred<T> {
	promise: Promise<T>;
	resolve!: (value: T | PromiseLike<T>) => void;
	reject!: (reason: Error) => void;
	pending = true;

	constructor() {
		this.promise = new Promise((resolve, reject) => {
			this.resolve = (value: T | PromiseLike<T>) => {
				this.pending = false;
				resolve(value);
			};
			this.reject = (reason: Error) => {
				this.pending = false;
				reject(reason);
			};
		});
	}
}

// @ts-ignore depends on the bundler
const dev = import.meta.env?.MODE !== "production";

export class WatchProducer<T> {
	#current: T;
	#next = new Deferred<T>();
	#closed = new Deferred<undefined>();
	#unused = new Deferred<undefined>();
	#epoch = 0;
	#consumers = 0;

	// Sanity check to make sure `close()` or `abort()` is being called.
	private static finalizer = new FinalizationRegistry<string>((debugInfo) => {
		console.warn(`WatchProducer was garbage collected without being closed:\n${debugInfo}`);
	});

	constructor(init: T) {
		this.#current = init;
		this.#unused.resolve(undefined);

		if (dev) {
			const debugInfo = new Error("Created here").stack ?? "No stack";
			WatchProducer.finalizer.register(this, debugInfo, this);
		}
	}

	value(): T {
		return this.#current;
	}

	async updated(): Promise<T | undefined> {
		return Promise.race([this.#next.promise, this.#closed.promise]);
	}

	update(v: T | ((v: T) => T)) {
		if (!this.#closed.pending) {
			throw new Error("closed");
		}

		let value: T;
		if (v instanceof Function) {
			value = v(this.#current);
		} else {
			value = v;
		}

		const next = new Deferred<T>();
		this.#current = value;
		this.#epoch++;
		this.#next.resolve(this.#current);
		this.#next = next;
	}

	close() {
		this.#closed.resolve(undefined);
		this.#unused.resolve(undefined);

		if (dev) {
			WatchProducer.finalizer.unregister(this);
		}
	}

	abort(reason: Error) {
		this.#closed.reject(reason);
		this.#unused.reject(reason);

		if (dev) {
			WatchProducer.finalizer.unregister(this);
		}
	}

	async closed(): Promise<void> {
		await this.#closed.promise;
	}

	async unused(): Promise<void> {
		await this.#unused.promise;
	}

	consume(): WatchConsumer<T> {
		if (this.#consumers === 0) {
			this.#unused = new Deferred<undefined>();
		}

		this.#consumers++;
		return new WatchConsumer(this);
	}

	unsubscribe() {
		this.#consumers--;

		if (this.#consumers === 0) {
			this.#unused.resolve(undefined);
		}
	}

	get epoch(): number {
		return this.#epoch;
	}
}

export class WatchConsumer<T> {
	#watch: WatchProducer<T>;

	// Used to make sure we don't process the same value twice.
	#epoch = 0;

	// Whether our specific consumer is closed.
	#closed = new Deferred<undefined>();

	// Sanity check to make sure `resolve()` or `reject()` is being called.
	private static finalizer = new FinalizationRegistry<string>((debugInfo) => {
		console.warn(`WatchConsumer was garbage collected without being closed:\n${debugInfo}`);
	});

	constructor(watch: WatchProducer<T>) {
		this.#watch = watch;

		if (dev) {
			const debugInfo = new Error("Created here").stack ?? "No stack";
			WatchConsumer.finalizer.register(this, debugInfo, this);
		}
	}

	value(): T {
		return this.#watch.value();
	}

	/// Returns the next value that hasn't been returned by `next` yet.
	/// Optionally, a filter can be provided to only return values that match the filter.
	async next(fn?: (v: T) => boolean): Promise<T | undefined> {
		if (!this.#closed.pending) {
			return undefined;
		}

		if (this.#epoch <= this.#watch.epoch) {
			this.#epoch = this.#watch.epoch + 1;

			const latest = this.value();
			if (!fn || fn(latest)) {
				return latest;
			}
		}

		for (;;) {
			const v = await Promise.race([this.#watch.updated(), this.#closed.promise]);
			this.#epoch = this.#watch.epoch + 1;

			if (v === undefined) {
				return undefined;
			}

			if (!fn || fn(v)) {
				return v;
			}
		}
	}

	/// Returns the next value that matches the filter.
	/// This will return the same value multiple times if it matches the filter.
	async when(fn: (v: T) => boolean): Promise<T | undefined> {
		if (!this.#closed.pending) {
			return undefined;
		}

		const latest = this.value();
		if (fn(latest)) {
			return latest;
		}

		for (;;) {
			const v = await Promise.race([this.#watch.updated(), this.#closed.promise]);
			if (v === undefined) {
				return undefined;
			}

			if (fn(v)) {
				return v;
			}
		}
	}

	clone(): WatchConsumer<T> {
		return this.#watch.consume();
	}

	close() {
		this.#closed.resolve(undefined);
		this.#watch.unsubscribe();
		if (dev) {
			WatchConsumer.finalizer.unregister(this);
		}
	}

	async closed(): Promise<void> {
		await Promise.race([this.#watch.closed(), this.#closed.promise]);
	}
}
