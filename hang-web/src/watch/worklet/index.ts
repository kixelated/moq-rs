import type { WorkletCommand } from "./message";

class Renderer extends AudioWorkletProcessor {
	base: number;

	constructor() {
		// The super constructor call is required.
		super();

		this.base = 0;
		this.port.onmessage = this.onMessage.bind(this);
	}

	onMessage(e: MessageEvent<WorkletCommand>) {
		const msg = e.data;
		if (msg.Frame) {
			this.onFrame(msg.Frame);
		}
	}

	onFrame(frame: AudioData) {}

	// Inputs and outputs in groups of 128 samples.
	process(_inputs: Float32Array[][], outputs: Float32Array[][], _parameters: Record<string, Float32Array>): boolean {
		console.log("process", _inputs, outputs, _parameters);
		return true;
	}
}

registerProcessor("renderer", Renderer);
