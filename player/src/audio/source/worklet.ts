// This is an AudioWorklet that acts as a media source.
// The renderer copies audio samples to a ring buffer read by this worklet.
// The worklet then outputs those samples to emit audio.

import * as Message from "../message"
import * as Util from "../../util"

import Ring from "./ring"

class Source extends AudioWorkletProcessor {
    channels?: Ring[];

    constructor() {
        // The super constructor call is required.
        super();

        this.port.onmessage = (e: MessageEvent) => {
            if (e.data.config) {
                this.config(e.data.config as Message.Config)
            }
        }
    }

    static get parameterDescriptors() {
        return [];
    }

    config(config: Message.Config) {
        this.channels = config.channels;
    }

    // TODO correct types
    process(inputs: any, outputs: any, parameters: any) {
        if (!this.channels) {
            return
        }

        if (outputs.length != 1) {
            throw new Error("only a single track is supported")
        }

        const track = outputs[0];

        for (let i = 0; i < track.length; i += 1) {
            const input = this.channels[i]
            const output = track[i];

            const parts = input.peek(output.length)

            let offset = 0
            for (let i = 0; i < parts.length; i += 1) {
                output.set(parts[i], offset)
                offset += parts[i].length
            }

            if (offset < output.length) {
                // TODO render silence
            }

            // Always advance the full amount.
            input.advance(output.length)
        }

        return true;
    }
}

self.registerProcessor("source", Source);