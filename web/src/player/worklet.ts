// This is an AudioWorklet that acts as a media source.
// The renderer copies audio samples to a ring buffer read by this worklet.
// The worklet then outputs those samples to emit audio.

import * as Message from "./message"

import { Ring } from "./ring"

class Renderer extends AudioWorkletProcessor {
	ring?: Ring
	base: number

	constructor(_params: AudioWorkletNodeOptions) {
		// The super constructor call is required.
		super()

		this.base = 0
		this.port.onmessage = this.onMessage.bind(this)
	}

	onMessage(e: MessageEvent) {
		if (e.data.play) {
			this.onPlay(e.data.play)
		}
	}

	onPlay(play: Message.Play) {
		this.ring = new Ring(play.buffer)
	}

	// Inputs and outputs in groups of 128 samples.
	process(inputs: Float32Array[][], outputs: Float32Array[][], _parameters: Record<string, Float32Array>): boolean {
		if (!this.ring) {
			// Paused
			return true
		}

		if (inputs.length != 1 && outputs.length != 1) {
			throw new Error("only a single track is supported")
		}

		const output = outputs[0]

		const size = this.ring.read(output)
		if (size < output.length) {
			// TODO trigger rebuffering event
		}

		return true
	}
}

registerProcessor("renderer", Renderer)
