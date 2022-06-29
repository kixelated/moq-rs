import { MP4New, MP4File, MP4ArrayBuffer, MP4Info } from "./mp4"

export class InitParser {
	mp4box: MP4File;
	offset: number;

	raw: MP4ArrayBuffer[];
	ready: Promise<Init>;

	constructor() {
		this.mp4box = MP4New()

		this.raw = []
		this.offset = 0

		// Create a promise that gets resolved once the init segment has been parsed.
		this.ready = new Promise((resolve, reject) => {
			this.mp4box.onError = reject

			// https://github.com/gpac/mp4box.js#onreadyinfo
			this.mp4box.onReady = (info: MP4Info) => {
				if (!info.isFragmented) {
					reject("expected a fragmented mp4")
				}

				if (info.tracks.length != 1) {
					reject("expected a single track")
				}

				resolve({
					info: info,
					raw: this.raw,
				})
			}
		})
	}

	push(data: Uint8Array) {
		// Make a copy of the atom because mp4box only accepts an ArrayBuffer unfortunately
		let box = new Uint8Array(data.byteLength);
		box.set(data)

		// and for some reason we need to modify the underlying ArrayBuffer with fileStart
		let buffer = box.buffer as MP4ArrayBuffer
		buffer.fileStart = this.offset

		// Parse the data
		this.offset = this.mp4box.appendBuffer(buffer)
		this.mp4box.flush()

		// Add the box to our queue of chunks
		this.raw.push(buffer)
	}
}

export interface Init {
	raw: MP4ArrayBuffer[];
	info: MP4Info;
}
