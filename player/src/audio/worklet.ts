// This is an AudioWorklet that acts as a media source.
// The renderer copies audio samples to a ring buffer read by this worklet.
// The worklet then outputs those samples to emit audio.

import * as Message from "./message"

import { Ring } from "./ring"

class Renderer extends AudioWorkletProcessor {
    ring?: Ring;
    base: number;

    constructor(params: AudioWorkletNodeOptions) {
        // The super constructor call is required.
        super();

        this.base = 0
        this.port.onmessage = this.onMessage.bind(this)
    }

    onMessage(e: MessageEvent) {
        if (e.data.config) {
            this.config(e.data.config)
        }
    }

    config(config: Message.Config) {
        this.ring = new Ring(config.ring)
    }

    // Inputs and outputs in groups of 128 samples.
    process(inputs: Float32Array[][], outputs: Float32Array[][], parameters: Record<string, Float32Array>): boolean {
        if (!this.ring) {
            // Not initialized yet
            return true
        }

        if (inputs.length != 1 && outputs.length != 1) {
            throw new Error("only a single track is supported")
        }

        const output = outputs[0]
        this.ring.read(output)

        return true;
    }
}

registerProcessor("renderer", Renderer);