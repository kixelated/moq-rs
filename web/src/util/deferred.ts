export default class Deferred<T> {
	promise: Promise<T>
	resolve: (value: T | PromiseLike<T>) => void
	reject: (value: T | PromiseLike<T>) => void

	constructor() {
		// Set initial values so TS stops being annoying.
		this.resolve = (_value: T | PromiseLike<T>) => {
			/* noop */
		}
		this.reject = (_value: T | PromiseLike<T>) => {
			/* noop */
		}

		this.promise = new Promise((resolve, reject) => {
			this.resolve = resolve
			this.reject = reject
		})
	}
}
