export class Context {
	done: Promise<never>;
	aborted: boolean;
	abort!: (reason?: unknown) => void;

	constructor() {
		this.aborted = false;

		this.done = new Promise((_, reject) => {
			this.abort = (reason?: unknown) => {
				this.aborted = true;
				reject(reason);
			};
		});
	}
}

export class Abortable<T> {
	#result: Promise<T>;
	#context: Context;
	#current?: T;

	constructor(fn: (context: Context) => Promise<T>) {
		this.#context = new Context();

		this.#result = fn(this.#context).then((value) => {
			this.#current = value;
			return value;
		});
	}

	abort(reason?: unknown) {
		this.#context.abort(reason);
	}

	get result(): Promise<T> {
		return this.#result;
	}

	get aborted(): boolean {
		return this.#context.aborted;
	}

	get current(): T | undefined {
		return this.#current;
	}
}
