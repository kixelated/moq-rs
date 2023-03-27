import * as Message from "./message";

export class Renderer {
    render: number; // non-zero if requestAnimationFrame has been called
    sync: DOMHighResTimeStamp; // the wall clock value for timestamp 0
    last?: number; // the timestamp of the last rendered frame

    constructor(config: Message.Config) {
        this.render = 0;
        this.sync = 0;
    }
}