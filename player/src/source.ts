import { Init } from "./init"

// Create a SourceBuffer with convenience methods
export class Source {
	sourceBuffer?: SourceBuffer;
	mediaSource: MediaSource;
	queue: Array<Uint8Array | ArrayBuffer>;
	mime: string;

	constructor(mediaSource: MediaSource) {
		this.mediaSource = mediaSource;
		this.queue = [];
		this.mime = "";
	}

	initialize(init: Init) {
		if (!this.sourceBuffer) {
			this.sourceBuffer = this.mediaSource.addSourceBuffer(init.info.mime)
			this.sourceBuffer.addEventListener('updateend', this.flush.bind(this))

			// Add the init data to the front of the queue
			for (let i = init.raw.length - 1; i >= 0; i -= 1) {
				this.queue.unshift(init.raw[i])
			}

			this.flush()
		} else if (init.info.mime != this.mime) {
			this.sourceBuffer.changeType(init.info.mime)

			// Add the init data to the front of the queue
			for (let i = init.raw.length - 1; i >= 0; i -= 1) {
				this.queue.unshift(init.raw[i])
			}
		}

		this.mime = init.info.mime
	}

	appendBuffer(data: Uint8Array | ArrayBuffer) {
		if (!this.sourceBuffer || this.sourceBuffer.updating || this.queue.length) {
			this.queue.push(data)
		} else {
			this.sourceBuffer.appendBuffer(data)
		}
	}

	buffered() {
		if (!this.sourceBuffer) {
			return { length: 0 }
		}

		return this.sourceBuffer.buffered
	}

	flush() {
		// Check if we have a mime yet
		if (!this.sourceBuffer) {
			return
		}

		// Check if the buffer is currently busy.
		if (this.sourceBuffer.updating) {
			return
		}

		const data = this.queue.shift()
		if (data) {
			// If there's data in the queue, flush it.
			this.sourceBuffer.appendBuffer(data)
		} else if (this.sourceBuffer.buffered.length) {
			// Otherwise with no data, trim anything older than 30s.
			const end = this.sourceBuffer.buffered.end(this.sourceBuffer.buffered.length - 1) - 30.0
			const start = this.sourceBuffer.buffered.start(0)

			// Remove any range larger than 1s.
			if (end > start && end - start > 1.0) {
				this.sourceBuffer.remove(start, end)
			}
		}
	}
}
