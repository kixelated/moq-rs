import * as Message from "./message"

// Wrapper around the WebWorker API
export default class Audio {
    worker: Worker;

    constructor(config: Message.Config) {
        this.worker = new Worker(new URL('worker.ts', import.meta.url), { type: "module" })
        this.worker.postMessage({ config }, [])
    }

    init(init: Message.Init) {
        this.worker.postMessage({ init }) // note: we copy the raw init bytes each time
    }

    segment(segment: Message.Segment) {
        this.worker.postMessage({ segment }, [ segment.buffer.buffer, segment.reader ])
    }
}