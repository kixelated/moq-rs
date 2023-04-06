import * as Message from "./message"
import { Ring } from "./ring"

export default class Renderer {
    ring: Ring;
    queue: Array<AudioData>;
    sync?: DOMHighResTimeStamp
    running: number;

    constructor(config: Message.Config) {
        this.ring = new Ring(config.ring)
        this.queue = [];
        this.running = 0
    }

    emit(frame: AudioData) {
        if (!this.sync) {
            // Save the frame as the sync point
            this.sync = 1000 * performance.now() - frame.timestamp
        }

        // Insert the frame into the queue sorted by timestamp.
        if (this.queue.length > 0 && this.queue[this.queue.length-1].timestamp <= frame.timestamp) {
            // Fast path because we normally append to the end.
            this.queue.push(frame)
        } else {
            // Do a full binary search
            let low = 0
            let high = this.queue.length;

            while (low < high) {
                var mid = (low + high) >>> 1;
                if (this.queue[mid].timestamp < frame.timestamp) low = mid + 1;
                else high = mid;
            }

            this.queue.splice(low, 0, frame)
        }

        if (!this.running) {
            // Wait for the next animation frame
            this.running = self.requestAnimationFrame(this.render.bind(this))
        }
    }

    render() {
        // Determine the target timestamp.
        const target = 1000 * performance.now() - this.sync!

        // Check if we should skip some frames
        while (this.queue.length) {
            const next = this.queue[0]
            if (next.timestamp >= target) {
                break
            }

            console.warn("dropping audio")

            this.queue.shift()
            next.close()
        }

        // Push as many as we can to the ring buffer.
        while (this.queue.length) {
            let frame = this.queue[0]
            let ok = this.ring.write(frame)
            if (!ok) {
                break
            }

            frame.close()
            this.queue.shift()
        }

        if (this.queue.length) {
            this.running = self.requestAnimationFrame(this.render.bind(this))
        } else {
            this.running = 0
        }
    }

    play(play: Message.Play) {
        this.ring.reset()
    }
}