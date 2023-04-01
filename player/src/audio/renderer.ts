import * as Message from "./message";

import Source from "./source";

export class Renderer {
    ctx: AudioContext;
    source: Source;

    render: number; // non-zero if requestAnimationFrame has been called
    last?: number; // the timestamp of the last rendered frame

    maxDuration: number; // the maximum duration allowed in the buffer

    constructor() {
        this.render = 0;
        this.maxDuration = 10 * 1000

        // TODO evaluate { latencyHint: "interactive" }
        this.ctx = new AudioContext()
        this.source = new Source(this.ctx)
    }

    emit(frame: AudioFrame) {
        this.source.emit(frame)
    }
}