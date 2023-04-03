import Decoder from "./decoder"

let decoder: Decoder

self.addEventListener('message', (e: MessageEvent) => {
    if (e.data.config) {
        decoder = new Decoder(e.data.config)
    }

    if (e.data.init) {
        decoder.init(e.data.init)
    }

    if (e.data.segment) {
        decoder.decode(e.data.segment)
    }
})