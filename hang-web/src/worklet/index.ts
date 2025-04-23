import type { WorkletCommand } from "./message";

type Frame = WorkletCommand["Frame"];

class Renderer extends AudioWorkletProcessor {
	#current: Frame | null = null;
	#current_pos = 0;
	#queued: Frame[] = [];

	constructor() {
		// The super constructor call is required.
		super();
		this.port.onmessage = this.onMessage.bind(this);
	}

	onMessage(e: MessageEvent<WorkletCommand>) {
		const msg = e.data;
		if (msg.Frame) {
			this.onFrame(msg.Frame);
		}
	}

	onFrame(frame: Frame) {
		if (this.#current === null) {
			this.#current = frame;
		} else if (this.#queued.length < 4) {
			this.#queued.push(frame);
		} else {
			console.warn(
				"frame buffer overflow, samples lost:",
				this.#queued.reduce((acc, f) => acc + f.channels[0].length, 0),
			);

			// Start the queue over to reset latency.
			this.#queued = [frame];
		}
	}

	// Inputs and outputs in groups of 128 samples.
	process(
		_inputs: Float32Array[][],
		outputs_all: Float32Array[][],
		_parameters: Record<string, Float32Array>,
	): boolean {
		if (this.#current === null) {
			return true;
		}

		// I don't know why, but the AudioWorkletProcessor interface gives us multiple outputs.
		const outputs = outputs_all[0];

		let offset = 0;

		// Keep looping until we've written the entire output buffer.
		while (this.#current !== null && offset < outputs[0].length) {
			let written = 0;

			// Loop over each channel and copy the current frame into the output buffer.
			for (let i = 0; i < Math.min(outputs.length, this.#current.channels.length); i++) {
				const output = outputs[i];
				const input = this.#current.channels[i];

				const current = input.subarray(
					this.#current_pos,
					Math.min(this.#current_pos + output.length - offset, input.length),
				);
				output.set(current, offset);

				// This will be the same value for every channel, so we're lazy.
				written = current.length;
			}

			// Advance the current position and offset.
			this.#current_pos += written;
			offset += written;

			// If we've reached the end of the current frame, advance to the next frame.
			if (this.#current_pos >= this.#current.channels[0].length) {
				this.#current_pos = 0;
				this.#current = this.#queued.shift() ?? null;
			}
		}

		if (offset < outputs[0].length) {
			console.warn("output buffer underrun, samples missing:", outputs[0].length - offset);
		}

		return true;
	}
}

registerProcessor("renderer", Renderer);
