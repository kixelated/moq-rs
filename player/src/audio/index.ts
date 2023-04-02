import * as Message from "./message"
import { Renderer } from "./renderer"
import { Decoder } from "./decoder"

export default class Audio {
    renderer: Renderer;
    decoder: Decoder;

    constructor(config: Message.Config) {
        this.renderer = new Renderer(config)
        this.decoder = new Decoder(config, this.renderer)
    }

    async init(init: Message.Init) {
        await this.decoder.init(init)
    }

    async segment(segment: Message.Segment) {
        await this.decoder.decode(segment)
    }
}