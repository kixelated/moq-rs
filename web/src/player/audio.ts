import * as Message from "./message"
import { Ring } from "./ring"

export default class Audio {
	ring?: Ring
	queue: Array<AudioData>

	render?: number // non-zero if requestAnimationFrame has been called
	last?: number // the timestamp of the last rendered frame, in microseconds

	constructor(_config: Message.Config) {
		this.queue = []
	}

	push(frame: AudioData) {
		// Drop any old frames
		if (this.last && frame.timestamp <= this.last) {
			frame.close()
			return
		}

		// Insert the frame into the queue sorted by timestamp.
		if (this.queue.length > 0 && this.queue[this.queue.length - 1].timestamp <= frame.timestamp) {
			// Fast path because we normally append to the end.
			this.queue.push(frame)
		} else {
			// Do a full binary search
			let low = 0
			let high = this.queue.length

			while (low < high) {
				const mid = (low + high) >>> 1
				if (this.queue[mid].timestamp < frame.timestamp) low = mid + 1
				else high = mid
			}

			this.queue.splice(low, 0, frame)
		}

		this.emit()
	}

	emit() {
		const ring = this.ring
		if (!ring) {
			return
		}

		while (this.queue.length) {
			const frame = this.queue[0]
			if (ring.size() + frame.numberOfFrames > ring.capacity) {
				// Buffer is full
				break
			}

			const size = ring.write(frame)
			if (size < frame.numberOfFrames) {
				throw new Error("audio buffer is full")
			}

			this.last = frame.timestamp

			frame.close()
			this.queue.shift()
		}
	}

	play(play: Message.Play) {
		this.ring = new Ring(play.buffer)

		if (!this.render) {
			const sampleRate = 44100 // TODO dynamic

			// Refresh every half buffer
			const refresh = ((play.buffer.capacity / sampleRate) * 1000) / 2
			this.render = setInterval(this.emit.bind(this), refresh)
		}
	}
}
