import * as Message from "./message"
import Ring from "./ring"

// Wrapper around the AudioWorklet API to make it easier to use.
export default class Source {
    ctx: AudioContext;
    worklet?: AudioWorkletNode; // async initialization
    channels: Ring[];

    ready: Promise<void>;

    constructor(ctx: AudioContext) {
        this.ctx = ctx

        // two channels, holding a maximum of 1s at 44khz
        this.channels = [
            new Ring(44000),
            new Ring(44000),
        ]

        // Start loading the worklet
        this.ready = this.setup()
    }

    private async setup(): Promise<void> {
        // Load the worklet source code.
        await this.ctx.audioWorklet.addModule('worklet.ts')

        // Create a worklet
        this.worklet = new AudioWorkletNode(this.ctx, 'source');

        this.worklet.port.onmessage = this.onMessage.bind(this)

        this.worklet.onprocessorerror = (e: Event) => {
            console.error("Audio worklet error:", e);
        };

        const config: Message.Config = {
            channels: this.channels,
        }

        this.worklet.port.postMessage({ config })
    }

    private async onMessage(e: MessageEvent) {
        if (e.data.configReply) {
            const reply = e.data.configReply as Message.ConfigReply

            if (reply.error) {
                throw reply.error
            }

            // Start playback
            this.worklet?.connect(this.ctx.destination);
        }
    }

    emit(frame: AudioFrame) {
        for (let i = 0; i < frame.channels; i += 1) {
            const ring = this.channels[i]
            ring.set(frame, i)
        }
    }
}