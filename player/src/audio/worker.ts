import { Renderer } from "./renderer"
import { Decoder } from "./decoder"
import * as Message from "./message"

let renderer: Renderer;
let decoder: Decoder;

self.addEventListener('message', async (e: MessageEvent) => {
    if (e.data.config) {
        const config = e.data.config as Message.Config;

        renderer = new Renderer(config)
        decoder = new Decoder(config, renderer)
    }

    if (e.data.init) {
        const init = e.data.init as Message.Init
        await decoder.init(init)
    }

    if (e.data.segment) {
        const segment = e.data.segment as Message.Segment
        await decoder.decode(segment)
    }
})
