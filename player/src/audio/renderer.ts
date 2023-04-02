import * as Message from "./message";

import Source from "./source";

export class Renderer {
    source: Source;

    render: number; // non-zero if requestAnimationFrame has been called
    last?: number; // the timestamp of the last rendered frame

    maxDuration: number; // the maximum duration allowed in the buffer

    constructor(config: Message.Config) {
        this.render = 0;
        this.maxDuration = 10 * 1000

        // TODO evaluate { latencyHint: "interactive" }
        this.source = new Source(config.ctx)
    }

    emit(frame: AudioData) {
        this.source.emit(frame)
    }
}