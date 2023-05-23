import * as Ring from "./ring"

export interface Config {
	// video stuff
	canvas: OffscreenCanvas
}

export interface Init {
	buffer: Uint8Array // unread buffered data
	reader: ReadableStream // unread unbuffered data
}

export interface Segment {
	buffer: Uint8Array // unread buffered data
	reader: ReadableStream // unread unbuffered data
}

export interface Play {
	timestamp?: number
	buffer: Ring.Buffer
}
