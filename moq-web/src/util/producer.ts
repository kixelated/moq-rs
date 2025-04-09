export class State<T> {
	#value: T;
	#notify: Promise<void>;

	#resolve!: () => void;
	#reject!: (reason: unknown) => void;

	constructor(init: T) {
		this.#value = init;
		this.#notify = new Promise((resolve, reject) => {
			this.#resolve = resolve;
			this.#reject = reject;
		});
	}

	set(value: T | ((value: T) => T)) {
		const notify = this.#resolve;
		this.#value = value instanceof Function ? value(this.#value) : value;
		this.#notify = new Promise((resolve, reject) => {
			this.#resolve = resolve;
			this.#reject = reject;
		});
		notify();
	}

	get(): T {
		return this.#value;
	}

	close(reason?: unknown) {
		this.#reject(reason);
	}

	wait(): Promise<void> {
		return this.#notify;
	}
}
