// Writer wraps a stream and writes chunks of data
export default class Writer {
	buffer: ArrayBuffer
	writer: WritableStreamDefaultWriter

	constructor(stream: WritableStream) {
		this.buffer = new ArrayBuffer(8)
		this.writer = stream.getWriter()
	}

	release() {
		this.writer.releaseLock()
	}

	async close() {
		return this.writer.close()
	}

	async uint8(v: number) {
		const view = new DataView(this.buffer, 0, 1)
		view.setUint8(0, v)
		return this.writer.write(view)
	}

	async uint16(v: number) {
		const view = new DataView(this.buffer, 0, 2)
		view.setUint16(0, v)
		return this.writer.write(view)
	}

	async uint24(v: number) {
		const v1 = (v >> 16) & 0xff
		const v2 = (v >> 8) & 0xff
		const v3 = v & 0xff

		const view = new DataView(this.buffer, 0, 3)
		view.setUint8(0, v1)
		view.setUint8(1, v2)
		view.setUint8(2, v3)

		return this.writer.write(view)
	}

	async uint32(v: number) {
		const view = new DataView(this.buffer, 0, 4)
		view.setUint32(0, v)
		return this.writer.write(view)
	}

	async uint52(v: number) {
		if (v > Number.MAX_SAFE_INTEGER) {
			throw "value too large"
		}

		this.uint64(BigInt(v))
	}

	async vint52(v: number) {
		if (v > Number.MAX_SAFE_INTEGER) {
			throw "value too large"
		}

		if (v < 1 << 6) {
			return this.uint8(v)
		} else if (v < 1 << 14) {
			return this.uint16(v | 0x4000)
		} else if (v < 1 << 30) {
			return this.uint32(v | 0x80000000)
		} else {
			return this.uint64(BigInt(v) | 0xc000000000000000n)
		}
	}

	async uint64(v: bigint) {
		const view = new DataView(this.buffer, 0, 8)
		view.setBigUint64(0, v)
		return this.writer.write(view)
	}

	async vint64(v: bigint) {
		if (v < 1 << 6) {
			return this.uint8(Number(v))
		} else if (v < 1 << 14) {
			return this.uint16(Number(v) | 0x4000)
		} else if (v < 1 << 30) {
			return this.uint32(Number(v) | 0x80000000)
		} else {
			return this.uint64(v | 0xc000000000000000n)
		}
	}

	async bytes(buffer: ArrayBuffer) {
		return this.writer.write(buffer)
	}

	async string(str: string) {
		const data = new TextEncoder().encode(str)
		return this.writer.write(data)
	}
}
