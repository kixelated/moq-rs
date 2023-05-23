import * as Message from "./message"
import * as Ring from "./ring"
import Transport from "../transport"

export interface Config {
	transport: Transport
	canvas: OffscreenCanvas
}

// This class must be created on the main thread due to AudioContext.
export default class Player {
	context: AudioContext
	worker: Worker
	worklet: Promise<AudioWorkletNode>

	transport: Transport

	constructor(config: Config) {
		this.transport = config.transport
		this.transport.callback = this

		this.context = new AudioContext({
			latencyHint: "interactive",
			sampleRate: 44100,
		})

		this.worker = this.setupWorker(config)
		this.worklet = this.setupWorklet(config)
	}

	private setupWorker(config: Config): Worker {
		const url = new URL("worker.ts", import.meta.url)

		const worker = new Worker(url, {
			type: "module",
			name: "media",
		})

		const msg = {
			canvas: config.canvas,
		}

		worker.postMessage({ config: msg }, [msg.canvas])

		return worker
	}

	private async setupWorklet(_config: Config): Promise<AudioWorkletNode> {
		// Load the worklet source code.
		const url = new URL("worklet.ts", import.meta.url)
		await this.context.audioWorklet.addModule(url)

		const volume = this.context.createGain()
		volume.gain.value = 2.0

		// Create a worklet
		const worklet = new AudioWorkletNode(this.context, "renderer")
		worklet.onprocessorerror = (e: Event) => {
			console.error("Audio worklet error:", e)
		}

		// Connect the worklet to the volume node and then to the speakers
		worklet.connect(volume)
		volume.connect(this.context.destination)

		return worklet
	}

	onInit(init: Message.Init) {
		this.worker.postMessage({ init }, [init.buffer.buffer, init.reader])
	}

	onSegment(segment: Message.Segment) {
		this.worker.postMessage({ segment }, [segment.buffer.buffer, segment.reader])
	}

	async play() {
		this.context.resume()

		const play = {
			buffer: new Ring.Buffer(2, 44100 / 10), // 100ms of audio
		}

		const worklet = await this.worklet
		worklet.port.postMessage({ play })
		this.worker.postMessage({ play })
	}
}
