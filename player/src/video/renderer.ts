import * as Message from "./message";

export class Renderer {
    canvas: OffscreenCanvas;
    queue: Array<VideoFrame>;
    render: number; // non-zero if requestAnimationFrame has been called
    sync: DOMHighResTimeStamp; // the wall clock value for timestamp 0
    last?: number; // the timestamp of the last rendered frame

    maxDuration: number; // the maximum duration allowed in the buffer

    constructor(config: Message.Config) {
        this.canvas = config.canvas;
        this.queue = [];
        this.render = 0;
        this.sync = 0;
        this.maxDuration = 10 * 1000
    }

    emit(frame: VideoFrame) {
        if (!this.sync) {
            // Save the frame as the sync point
            this.sync = performance.now() - frame.timestamp
        }

        // Drop any old frames
        if (this.last && frame.timestamp <= this.last) {
            frame.close()
            return
        }

        // Insert the frame into the queue sorted by timestamp.
        let low = 0
        let high = this.queue.length;

        // Fast path because we normally append to the end.
        if (this.queue.length > 0 && this.queue[this.queue.length].timestamp <= frame.timestamp) {
            this.queue.push(frame)
        } else {
            // Do a full binary search
            while (low < high) {
                var mid = (low + high) >>> 1;
                if (this.queue[mid].timestamp < frame.timestamp) low = mid + 1;
                else high = mid;
            }

            this.queue.splice(low, 0, frame)
        }

        // Trim the max size of the buffer
        const last = this.queue[this.queue.length-1].timestamp
        while (1) {
            const first = this.queue[0]
            if (first.timestamp + this.maxDuration >= last) {
                break
            }

            first.close()
            this.queue.shift()
        }

        // Queue up to render the next frame.
        if (!this.render) {
            this.render = self.requestAnimationFrame(this.draw.bind(this))
        }
    }

    draw(now: DOMHighResTimeStamp) {
        // Determine the target timestamp.
        const target = now - this.sync

        let frame = this.queue[0]
        if (frame.timestamp > target) {
            // nothing to render yet, wait for the next animation frame
            this.render = self.requestAnimationFrame(this.draw.bind(this))
            return
        }

        this.queue.shift()

        // Check if we should skip some frames
        while (this.queue.length) {
            const next = this.queue[0]
            if (next.timestamp > target) {
                break
            }

            frame.close()

            this.queue.shift()
            frame = next
        }

        const ctx = this.canvas.getContext("2d");
        ctx?.drawImage(frame, 0, 0, this.canvas.width, this.canvas.height) // TODO aspect ratio

        this.last = frame.timestamp;
        frame.close()

        if (this.queue.length > 0) {
            this.render = self.requestAnimationFrame(this.draw.bind(this))
        } else {
            // Break the loop for now
            this.render = 0
        }
    }
}