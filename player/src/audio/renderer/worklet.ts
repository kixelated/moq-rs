// This is an AudioWorklet that acts as a media source.
// The renderer copies audio samples to a ring buffer read by this worklet.
// The worklet then outputs those samples to emit audio.

import * as Message from "../message"

import { Ring, RingState } from "../ring"

class Renderer extends AudioWorkletProcessor {
    channels?: Ring[];

    constructor(params: AudioWorkletNodeOptions) {
        // The super constructor call is required.
        super();

        this.port.onmessage = this.onMessage.bind(this)
    }

    onMessage(e: MessageEvent) {
        if (e.data.config) {
            this.config(e.data.config)
        }
    }

    config(config: Message.Config) {
        this.channels = []
        for (let state of config.channels) {
            this.channels.push(new Ring(state))
        }
    }

    // Inputs and outputs in groups of 128 samples.
    process(inputs: Float32Array[][], outputs: Float32Array[][], parameters: Record<string, Float32Array>): boolean {
        if (!this.channels) {
            // Not initialized yet
            return true
        }

        if (inputs.length != 1 && outputs.length != 1) {
            throw new Error("only a single track is supported")
        }

        const output = outputs[0]

        for (let i = 0; i < output.length; i += 1) {
            const source = this.channels[i]
            const channel = output[i];

            const parts = source.peek(channel.length)

            let offset = 0
            for (let i = 0; i < parts.length; i += 1) {
                channel.set(parts[i], offset)
                offset += parts[i].length
            }

            if (offset < channel.length) {
                // TODO render silence
            }

            // Always advance the full amount.
            source.advance(channel.length)
        }

        return true;
    }
}

registerProcessor("renderer", Renderer);