export default class Deferred<T> {
    promise: Promise<T>
    resolve: (value: T | PromiseLike<T>) => void
    reject: (value: T | PromiseLike<T>) => void
  
    constructor() {
        // Set initial values so TS stops being annoying.
        this.resolve = (value: T | PromiseLike<T>) => {};
        this.reject = (value: T | PromiseLike<T>) => {};

        this.promise = new Promise((resolve, reject) => {
            this.resolve = resolve
            this.reject = reject
        })
    }
  }