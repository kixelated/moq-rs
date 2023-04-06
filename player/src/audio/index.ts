import * as Message from "./message"
import Renderer from "./renderer"
import Decoder from "./decoder"

import { RingInit } from "./ring"

// Abstracts the Worker and Worklet into a simpler API
// This class must be created on the main thread due to AudioContext.
export default class Audio {
    context: AudioContext;
    worker: Worker;
    worklet: Promise<AudioWorkletNode>;

    constructor() {
        // Assume 44.1kHz and two audio channels
        const config = {
            sampleRate: 44100,
            ring: new RingInit(2, 4410), // 100ms at 44.1khz
        }

        this.context = new AudioContext({
            latencyHint: "interactive",
            sampleRate: config.sampleRate,
        })

        this.worker = this.setupWorker(config)
        this.worklet = this.setupWorklet(config)
    }

    private setupWorker(config: Message.Config): Worker {
        const url = new URL('worker.ts', import.meta.url)
        const worker = new Worker(url, {
            name: "audio",
            type: "module",
        })

        worker.postMessage({ config })

        return worker
    }

    private async setupWorklet(config: Message.Config): Promise<AudioWorkletNode> {
        // Load the worklet source code.
        const url = new URL('worklet.ts', import.meta.url)
        await this.context.audioWorklet.addModule(url)

        const volume = this.context.createGain()
        volume.gain.value = 2.0;

        // Create a worklet
        const worklet = new AudioWorkletNode(this.context, 'renderer');
        worklet.onprocessorerror = (e: Event) => {
            console.error("Audio worklet error:", e)
        };

        worklet.port.postMessage({ config })

        // Connect the worklet to the volume node and then to the speakers
        worklet.connect(volume)
        volume.connect(this.context.destination)

        return worklet
    }

    init(init: Message.Init) {
        this.worker.postMessage({ init })
    }

    segment(segment: Message.Segment) {
        this.worker.postMessage({ segment }, [ segment.buffer.buffer, segment.reader ])
    }

    play(play: Message.Play) {
        this.context.resume()
        //this.worker.postMessage({ play })
    }
}