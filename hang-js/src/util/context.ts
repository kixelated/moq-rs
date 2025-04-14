export type Reason = string | Error;

// A way to cancel async operations.
//
// Async functions take a context and keep executing until it resolves (via abort).
// This is very similar to AbortSignal, but the API is not ass.
export class Context {
	#aborted: Promise<never>;
	#abort!: (reason?: Reason) => void;

	constructor() {
		this.#aborted = new Promise((_, reject) => {
			this.#abort = reject;
		});
	}

	abort(reason?: Reason) {
		this.#abort(reason);
	}

	get done(): Promise<Reason | undefined> {
		return this.#aborted.catch((reason) => reason);
	}

	async all<T>(value: Promise<T>): Promise<T>;
	async all<T extends readonly unknown[] | []>(values: T): Promise<{ -readonly [P in keyof T]: Awaited<T[P]> }>;
	async all<T>(values: Promise<T> | readonly unknown[] | []): Promise<Awaited<T>> {
		const valuesArray = Array.isArray(values) ? values : [values];
		const result = await Promise.all([...valuesArray, this.#aborted]);
		return result.at(-1);
	}

	async race<T>(value: Promise<T>): Promise<T>;
	async race<T extends readonly unknown[] | []>(values: T): Promise<Awaited<T[number]>>;
	async race<T>(values: Promise<T> | readonly unknown[] | []): Promise<Awaited<T>> {
		const valuesArray = Array.isArray(values) ? values : [values];
		return await Promise.race([...valuesArray, this.#aborted]);
	}
}

// An async promise with an associated context.
//
// This is a way to retry async operations (with different arguments), cancelling the previous operation if needed.
export class Task<T, Args extends unknown[] = unknown[]> {
	#fn: (context: Context, ...args: Args) => Promise<T>;
	#latest?: Run<T, Args>;

	constructor(fn: (context: Context, ...args: Args) => Promise<T>) {
		this.#fn = fn;
	}

	// Starts a new execution of the function.
	//
	// Blocks until the previous execution has been aborted/finished.
	async start(...args: Args): Promise<Run<T, Args>> {
		await this.abort();

		const run = new Run(this.#fn, ...args);
		this.#latest = run;

		return run;
	}

	// Blocks until the previous task has finished.
	async abort(reason?: Reason): Promise<void> {
		this.#latest?.abort(reason);

		// Wait for the task to finish, erroring after 2 seconds.
		const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error("task failed to abort")), 2_000));
		await Promise.race([this.#latest?.promise, timeout]);
	}

	get latest(): Run<T, Args> | undefined {
		return this.#latest;
	}
}

// A wrapper around a Promise that can be aborted via the context.
export class Run<T, Args extends unknown[] = unknown[]> {
	#context: Context;
	#result?: T;

	readonly promise: Promise<T>;

	constructor(fn: (context: Context, ...args: Args) => Promise<T>, ...args: Args) {
		this.#context = new Context();
		this.promise = fn(this.#context, ...args).then((result) => {
			this.#result = result;
			return result;
		});
	}

	// Blocks until the function has finished.
	async abort(reason?: Reason): Promise<void> {
		this.#context.abort(reason);

		// Wait for the task to finish, erroring after 2 seconds.
		const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error("task failed to abort")), 2_000));
		await Promise.race([this.promise, timeout]);
	}

	get result(): T | undefined {
		return this.#result;
	}

	// Helper methods to avoid using the promise directly.

	// biome-ignore lint/suspicious/noThenProperty: This is a wrapper around the promise.
	then<TResult1 = T, TResult2 = never>(
		onfulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | undefined | null,
		onrejected?: ((reason: unknown) => TResult2 | PromiseLike<TResult2>) | undefined | null,
	): Promise<TResult1 | TResult2> {
		return this.promise.then(onfulfilled, onrejected);
	}

	catch<TResult = never>(
		onrejected?: ((reason: unknown) => TResult | PromiseLike<TResult>) | undefined | null,
	): Promise<T | TResult> {
		return this.promise.catch(onrejected);
	}

	finally(fn: () => void) {
		this.promise.finally(fn);
	}
}
