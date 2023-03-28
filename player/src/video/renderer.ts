import * as Message from "./message";

export class Renderer {
    canvas: OffscreenCanvas;
    queue: Array<VideoFrame>;
    render: number; // non-zero if requestAnimationFrame has been called
    sync: DOMHighResTimeStamp; // the wall clock value for timestamp 0
    last?: number; // the timestamp of the last rendered frame

    constructor(config: Message.Config) {
        this.canvas = config.canvas;
        this.queue = [];
        this.render = 0;
        this.sync = 0;
    }

    push(frame: VideoFrame) {
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
        // TODO loop backwards for better performance
        let index = this.queue.findIndex(other => {
            return frame.timestamp < other.timestamp;
        })

        // Insert into the queue.
        this.queue.splice(index, 0, frame)

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