import * as Message from "./message";
import { Ring } from "./ring"

export default class Renderer {
    audioRing: Ring;
    audioQueue: Array<AudioData>;

    videoCanvas: OffscreenCanvas;
    videoQueue: Array<VideoFrame>;

    render: number; // non-zero if requestAnimationFrame has been called
    sync?: DOMHighResTimeStamp; // the wall clock value for timestamp 0, in microseconds
    last?: number; // the timestamp of the last rendered frame, in microseconds

    constructor(config: Message.Config) {
        this.audioRing = new Ring(config.audio.ring);
        this.audioQueue = [];

        this.videoCanvas = config.video.canvas;
        this.videoQueue = [];

        this.render = 0;
    }

    push(frame: AudioData | VideoFrame) {
        if (!this.sync) {
            // Save the frame as the sync point
            this.sync = 1000 * performance.now() - frame.timestamp
        }

        // Drop any old frames
        if (this.last && frame.timestamp <= this.last) {
            frame.close()
            return
        }

        let queue
        if (isAudioData(frame)) {
            queue = this.audioQueue;
        } else if (isVideoFrame(frame)) {
            queue = this.videoQueue;
        } else {
            throw new Error("unknown frame type")
        }

        // Insert the frame into the queue sorted by timestamp.
        if (queue.length > 0 && queue[queue.length-1].timestamp <= frame.timestamp) {
            // Fast path because we normally append to the end.
            queue.push(frame as any)
        } else {
            // Do a full binary search
            let low = 0
            let high = queue.length;

            while (low < high) {
                var mid = (low + high) >>> 1;
                if (queue[mid].timestamp < frame.timestamp) low = mid + 1;
                else high = mid;
            }

            queue.splice(low, 0, frame as any)
        }

        // Queue up to render the next frame.
        if (!this.render) {
            this.render = self.requestAnimationFrame(this.draw.bind(this))
        }
    }

    draw(now: DOMHighResTimeStamp) {
        // Determine the target timestamp.
        const target = 1000 * now - this.sync!

        this.drawAudio(now, target)
        this.drawVideo(now, target)

        if (this.audioQueue.length || this.videoQueue.length) {
            this.render = self.requestAnimationFrame(this.draw.bind(this))
        } else {
            this.render = 0
        }
    }

    drawAudio(now: DOMHighResTimeStamp, target: DOMHighResTimeStamp) {
        // Check if we should skip some frames
        while (this.audioQueue.length) {
            const next = this.audioQueue[0]
            if (next.timestamp >= target) {
                let ok = this.audioRing.write(next)
                if (!ok) {
                    // No more space in the ring
                    break
                }
            } else {
                console.warn("dropping audio")
            }

            next.close()
            this.audioQueue.shift()
        }
    }

    drawVideo(now: DOMHighResTimeStamp, target: DOMHighResTimeStamp) {
        if (this.videoQueue.length == 0) return;

        let frame = this.videoQueue[0];
        if (frame.timestamp >= target) {
            // nothing to render yet, wait for the next animation frame
            this.render = self.requestAnimationFrame(this.draw.bind(this))
            return
        }

        this.videoQueue.shift();

        // Check if we should skip some frames
        while (this.videoQueue.length) {
            const next = this.videoQueue[0]
            if (next.timestamp > target) break

            frame.close()
            frame = this.videoQueue.shift()!;
        }

        const ctx = this.videoCanvas.getContext("2d");
        ctx!.drawImage(frame, 0, 0, this.videoCanvas.width, this.videoCanvas.height) // TODO aspect ratio

        this.last = frame.timestamp;
        frame.close()
    }
}

function isAudioData(frame: AudioData | VideoFrame): frame is AudioData {
    return frame instanceof AudioData
}

function isVideoFrame(frame: AudioData | VideoFrame): frame is VideoFrame {
    return frame instanceof VideoFrame
}