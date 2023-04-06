import * as Message from "./message"

// Wrapper around the WebWorker API
export default class Video {
    worker: Worker;

    constructor(config: Message.Config) {
        const url = new URL('worker.ts', import.meta.url)
        this.worker = new Worker(url, {
            type: "module",
            name: "video",
        })
        this.worker.postMessage({ config }, [ config.canvas ])
    }

    init(init: Message.Init) {
        this.worker.postMessage({ init }) // note: we copy the raw init bytes each time
    }

    segment(segment: Message.Segment) {
        this.worker.postMessage({ segment }, [ segment.buffer.buffer, segment.reader ])
    }

    play() {
        // TODO
    }
}