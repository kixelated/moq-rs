import * as Message from "./message"
import Renderer from "./renderer"
import Decoder from "./decoder"
import { RingState } from "./ring"

// Abstracts the Worker and Worklet into a simpler API
// This class must be created on the main thread due to AudioContext.
export default class Audio {
    decoder: Decoder;   // WebWorker
    renderer: Renderer; // AudioWorklet

    constructor() {
        // Assume 44.1kHz and two audio channels
        const config = {
            sampleRate: 44100,
            channels: [ new RingState(44100), new RingState(44100) ],
        }

        // Start loading the worker script
        this.decoder = new Decoder(config)
        this.renderer = new Renderer(config)
    }

    init(init: Message.Init) {
        this.decoder.init(init)
    }

    segment(segment: Message.Segment) {
        this.decoder.segment(segment)
    }

    play() {
        this.renderer.play()
    }
}