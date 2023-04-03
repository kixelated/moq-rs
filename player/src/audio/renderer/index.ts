import * as Message from "../message"

export default class Renderer {
    context: AudioContext;
    worklet: Promise<AudioWorkletNode>;

    constructor(config: Message.Config) {
        this.context = new AudioContext({
            latencyHint: "interactive",
            sampleRate: config.sampleRate,
        })

        this.worklet = this.setup(config)
    }

    private async setup(config: Message.Config): Promise<AudioWorkletNode> {
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

        worklet.port.onmessage = this.onMessage.bind(this)
        worklet.port.postMessage({ config })

        // Connect the worklet to the volume node and then to the speakers
        worklet.connect(volume)
        volume.connect(this.context.destination)

        return worklet
    }

    private onMessage(e: MessageEvent) {
        // TODO
    }

    play() {
        this.context.resume()
    }
}