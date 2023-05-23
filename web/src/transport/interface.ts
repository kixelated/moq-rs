export interface Callback {
	onInit(init: Init): any
	onSegment(segment: Segment): any
}

export interface Init {
	buffer: Uint8Array // unread buffered data
	reader: ReadableStream // unread unbuffered data
}

export interface Segment {
	buffer: Uint8Array // unread buffered data
	reader: ReadableStream // unread unbuffered data
}
