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

export class Watch<T> {
	readonly producer: WatchProducer<T>;
	readonly consumer: WatchConsumer<T>;

	constructor(init: T) {
		this.producer = new WatchProducer<T>(init);
		this.consumer = this.producer.subscribe();
	}
}

export class WatchProducer<T> {
	#current: T;
	#next = new Deferred<T>();
	#closed = new Deferred<undefined>();

	#consumers = 0;

	constructor(init: T) {
		this.#current = init;
	}

	latest(): T {
		return this.#current;
	}

	async next(): Promise<T | undefined> {
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
		this.#next.resolve(this.#current);
		this.#next = next;
	}

	close() {
		this.#closed.resolve(undefined);
	}

	abort(reason: Error) {
		this.#closed.reject(reason);
	}

	async closed(): Promise<void> {
		await this.#closed.promise;
	}

	subscribe(): WatchConsumer<T> {
		this.#consumers++;
		return new WatchConsumer(this);
	}

	unsubscribe() {
		this.#consumers--;
		if (this.#consumers === 0) {
			this.close();
		}
	}
}

export class WatchConsumer<T> {
	#watch: WatchProducer<T>;

	// Whether our specific consumer is closed.
	#closed = new Deferred<undefined>();

	constructor(watch: WatchProducer<T>) {
		this.#watch = watch;
	}

	latest(): T {
		return this.#watch.latest();
	}

	async next(): Promise<T | undefined> {
		return Promise.race([this.#watch.next(), this.#closed.promise]);
	}

	async when(fn: (v: T) => boolean): Promise<T | undefined> {
		if (!this.#closed.pending) {
			return undefined;
		}

		const latest = this.latest();
		if (fn(latest)) {
			return latest;
		}

		for (;;) {
			const v = await this.next();
			if (v === undefined) {
				return undefined;
			}

			if (fn(v)) {
				return v;
			}
		}
	}

	clone(): WatchConsumer<T> {
		return this.#watch.subscribe();
	}

	close() {
		this.#closed.resolve(undefined);
		this.#watch.unsubscribe();
	}

	async closed(): Promise<void> {
		await Promise.race([this.#watch.closed(), this.#closed.promise]);
	}
}
