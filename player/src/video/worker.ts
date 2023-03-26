import { Renderer } from "./renderer"
import { Decoder } from "./decoder"
import * as Message from "./message"

let decoder: Decoder;
let renderer: Renderer;

self.addEventListener('message', async (e: MessageEvent) => {
    if (e.data.config) {
        const config = e.data.config as Message.Config

        renderer = new Renderer(config)
        decoder = new Decoder(renderer)
    }

    if (e.data.segment) {
        const segment = e.data.segment as Message.Segment

        await decoder.decode(segment)
    }
})

