import * as Message from "./message";
import { Ring } from "./ring"

export default class Audio {
    ring: Ring;
    queue: Array<AudioData>;

    sync?: DOMHighResTimeStamp; // the wall clock value for timestamp 0, in microseconds
    last?: number; // the timestamp of the last rendered frame, in microseconds

    constructor(config: Message.AudioConfig) {
        this.ring = new Ring(config.ring);
        this.queue = [];
    }

    push(frame: AudioData) {
        if (!this.sync) {
            // Save the frame as the sync point
			// TODO sync with video
            this.sync = 1000 * performance.now() - frame.timestamp
        }

        // Drop any old frames
        if (this.last && frame.timestamp <= this.last) {
            frame.close()
            return
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
                const mid = (low + high) >>> 1;
                if (this.queue[mid].timestamp < frame.timestamp) low = mid + 1;
                else high = mid;
            }

            this.queue.splice(low, 0, frame)
        }
    }


    draw() {
        // Convert to microseconds
        const now = 1000 * performance.now();

        // Determine the target timestamp.
        const target = now - this.sync!

        // Check if we should skip some frames
        while (this.queue.length) {
            const next = this.queue[0]

            if (next.timestamp > target) {
                const ok = this.ring.write(next)
                if (!ok) {
                    console.warn("ring buffer is full")
                    // No more space in the ring
                    break
                }
            } else {
                console.warn("dropping audio")
            }

            next.close()
            this.queue.shift()
        }
    }
}