export class Deferred<T> {
	promise: Promise<T>;
	resolve!: (value: T | PromiseLike<T>) => void;
	reject!: (reason: unknown) => void;
	pending = true;

	constructor() {
		this.promise = new Promise((resolve, reject) => {
			this.resolve = (value) => {
				this.pending = false;
				resolve(value);
			};
			this.reject = (reason) => {
				this.pending = false;
				reject(reason);
			};
		});
	}
}

export class WatchProducer<T> {
	#current?: T;
	#next = new Deferred<T>();
	#closed = new Deferred<undefined>();

	#consumers = 0;

	latest(): T | undefined {
		return this.#current;
	}

	async next(): Promise<T | undefined> {
		return Promise.race([this.#next.promise, this.#closed.promise]);
	}

	update(v: T | ((v: T | undefined) => T)) {
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

	abort(reason?: unknown) {
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

	static pair<T>(): [WatchProducer<T>, WatchConsumer<T>] {
		const producer = new WatchProducer<T>();
		const consumer = producer.subscribe();
		return [producer, consumer];
	}
}

export class WatchConsumer<T> {
	#watch: WatchProducer<T>;

	constructor(watch: WatchProducer<T>) {
		this.#watch = watch;
	}

	async next(): Promise<T | undefined> {
		// Return the next value
		return this.#watch.next();
	}

	clone(): WatchConsumer<T> {
		return this.#watch.subscribe();
	}

	close() {
		this.#watch.unsubscribe();
	}
}
