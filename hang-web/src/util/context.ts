export class Context {
	#aborted: Promise<never>;
	#abort!: (reason?: unknown) => void;

	constructor() {
		this.#aborted = new Promise((_, reject) => {
			this.#abort = reject;
		});
	}

	abort(reason?: unknown) {
		this.#abort(reason);
	}

	async run<T>(promise: Promise<T>): Promise<T> {
		return await Promise.race([promise, this.#aborted]);
	}

	async race<T extends readonly unknown[] | []>(values: T): Promise<Awaited<T[number]>> {
		return await Promise.race([...values, this.#aborted]);
	}
}

export class Task<Args extends unknown[] = unknown[]> {
	#context?: Context;
	#fn: (context: Context, ...args: Args) => Promise<void>;
	#active?: Promise<void>;

	constructor(fn: (context: Context, ...args: Args) => Promise<void>) {
		this.#fn = fn;
	}

	async start(context: Context, ...args: Args): Promise<void> {
		await this.abort();

		this.#context = context;
		this.#active = this.#fn(this.#context, ...args).catch((error) => {
			this.#context = undefined;
			throw error;
		});

		return this.#active;
	}

	async abort(reason?: unknown) {
		this.#context?.abort(reason);
		// Wait for the task to finish, erroring after 2 seconds.
		const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error("task failed to abort")), 2_000));
		await Promise.race([this.#active, timeout]);
	}
}
