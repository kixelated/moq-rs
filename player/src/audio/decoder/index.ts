import * as Message from "../message"

// Wrapper to run the decoder in a Worker
export default class Decoder {
    worker: Worker;

    constructor(config: Message.Config) {
        const url = new URL('worker.ts', import.meta.url)
        this.worker = new Worker(url, {
            name: "audio",
            type: "module",
        })

        this.worker.onmessage = this.onMessage.bind(this)
        this.worker.postMessage({ config })
    }

    init(init: Message.Init) {
        this.worker.postMessage({ init })
    }

    segment(segment: Message.Segment) {
        this.worker.postMessage({ segment }, [ segment.buffer.buffer, segment.reader ])
    }

    private onMessage(e: MessageEvent) {
        // TODO
    }
}
