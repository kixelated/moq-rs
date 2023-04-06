import Decoder from "./decoder"
import Renderer from "./renderer"

import * as Message from "./message"

let decoder: Decoder
let renderer: Renderer;

self.addEventListener('message', (e: MessageEvent) => {
    if (e.data.config) {
        renderer = new Renderer(e.data.config)
        decoder = new Decoder(e.data.config, renderer)
    }

    if (e.data.init) {
        decoder.init(e.data.init)
    }

    if (e.data.segment) {
        decoder.decode(e.data.segment)
    }

    if (e.data.play) {
        renderer.play(e.data.play)
    }
})