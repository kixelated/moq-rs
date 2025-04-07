export class Context {
	#aborted: Promise<void>;
	#abort!: () => void;

	constructor() {
		this.#aborted = new Promise((resolve, _) => {
			this.#abort = resolve;
		});
	}

	abort() {
		this.#abort();
	}

	get done(): Promise<void> {
		return this.#aborted;
	}

	async all<T>(value: Promise<T>): Promise<T>;
	async all<T extends readonly unknown[] | []>(values: T): Promise<{ -readonly [P in keyof T]: Awaited<T[P]> }>;
	async all<T>(values: Promise<T> | readonly unknown[] | []): Promise<Awaited<T>> {
		const valuesArray = Array.isArray(values) ? values : [values];
		const result = await Promise.all([...valuesArray, this.#aborted]);
		return result.at(-1);
	}

	async race<T>(value: Promise<T>): Promise<T | undefined>;
	async race<T extends readonly unknown[] | []>(values: T): Promise<Awaited<T[number]> | undefined>;
	async race<T>(values: Promise<T> | readonly unknown[] | []): Promise<Awaited<T> | undefined> {
		const valuesArray = Array.isArray(values) ? values : [values];
		return await Promise.race([...valuesArray, this.#aborted.then(() => undefined)]);
	}
}

export class Task<T, Args extends unknown[] = unknown[]> {
	#context?: Context;
	#fn: (context: Context, ...args: Args) => Promise<T>;
	#active?: Promise<T>;
	#value?: T;

	constructor(fn: (context: Context, ...args: Args) => Promise<T>) {
		this.#fn = fn;
	}

	async start(...args: Args): Promise<T> {
		await this.abort();

		this.#context = new Context();
		this.#active = this.#fn(this.#context, ...args)
			.then((value) => {
				this.#value = value;
				return value;
			})
			.catch((error) => {
				this.#context = undefined;
				this.#value = undefined;
				throw error;
			});

		return this.#active;
	}

	async abort() {
		this.#context?.abort();

		// Wait for the task to finish, erroring after 2 seconds.
		const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error("task failed to abort")), 2_000));
		await Promise.race([this.#active, timeout]);
	}

	get value(): T | undefined {
		return this.#value;
	}
}
