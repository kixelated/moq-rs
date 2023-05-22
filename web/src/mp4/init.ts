import * as MP4 from "./index"

export interface Init {
	raw: MP4.ArrayBuffer;
	info: MP4.Info;
}

export class InitParser {
	mp4box: MP4.File;
	offset: number;

	raw: MP4.ArrayBuffer[];
	info: Promise<MP4.Info>;

	constructor() {
		this.mp4box = MP4.New()
		this.raw = []
		this.offset = 0

		// Create a promise that gets resolved once the init segment has been parsed.
		this.info = new Promise((resolve, reject) => {
			this.mp4box.onError = reject
			this.mp4box.onReady = resolve
		})
	}

	push(data: Uint8Array) {
		// Make a copy of the atom because mp4box only accepts an ArrayBuffer unfortunately
		const box = new Uint8Array(data.byteLength);
		box.set(data)

		// and for some reason we need to modify the underlying ArrayBuffer with fileStart
		const buffer = box.buffer as MP4.ArrayBuffer
		buffer.fileStart = this.offset

		// Parse the data
		this.offset = this.mp4box.appendBuffer(buffer)
		this.mp4box.flush()

		// Add the box to our queue of chunks
		this.raw.push(buffer)
	}
}