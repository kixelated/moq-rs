// Reader wraps a stream and provides convience methods for reading pieces from a stream
export class StreamReader {
	reader: ReadableStreamDefaultReader; // TODO make a separate class without promises when null
	buffer: Uint8Array;

	constructor(reader: ReadableStreamDefaultReader, buffer: Uint8Array = new Uint8Array(0)) {
		this.reader = reader
		this.buffer = buffer
	}

	// TODO implementing pipeTo seems more reasonable than releasing the lock
	release() {
		this.reader.releaseLock()
	}

	// Returns any number of bytes
	async read(): Promise<Uint8Array | undefined> {
		if (this.buffer.byteLength) {
			const buffer = this.buffer;
			this.buffer = new Uint8Array()
			return buffer
		}

		const result = await this.reader.read()
		return result.value
	}

	async bytes(size: number): Promise<Uint8Array> {
		while (this.buffer.byteLength < size) {
			const result = await this.reader.read()
			if (result.done) {
				throw "short buffer"
			}

			const buffer = new Uint8Array(result.value)

			if (this.buffer.byteLength == 0) {
				this.buffer = buffer
			} else {
				const temp = new Uint8Array(this.buffer.byteLength + buffer.byteLength)
				temp.set(this.buffer)
				temp.set(buffer, this.buffer.byteLength)
				this.buffer = temp
			}
		}

		const result = new Uint8Array(this.buffer.buffer, this.buffer.byteOffset, size)
		this.buffer = new Uint8Array(this.buffer.buffer, this.buffer.byteOffset + size)

		return result
	}

	async peek(size: number): Promise<Uint8Array> {
		while (this.buffer.byteLength < size) {
			const result = await this.reader.read()
			if (result.done) {
				throw "short buffer"
			}

			const buffer = new Uint8Array(result.value)

			if (this.buffer.byteLength == 0) {
				this.buffer = buffer
			} else {
				const temp = new Uint8Array(this.buffer.byteLength + buffer.byteLength)
				temp.set(this.buffer)
				temp.set(buffer, this.buffer.byteLength)
				this.buffer = temp
			}
		}

		return new Uint8Array(this.buffer.buffer, this.buffer.byteOffset, size)
	}

	async view(size: number): Promise<DataView> {
		const buf = await this.bytes(size)
		return new DataView(buf.buffer, buf.byteOffset, buf.byteLength)
	}

	async uint8(): Promise<number> {
		const view = await this.view(1)
		return view.getUint8(0)
	}

	async uint16(): Promise<number> {
		const view = await this.view(2)
		return view.getUint16(0)
	}

	async uint32(): Promise<number> {
		const view = await this.view(4)
		return view.getUint32(0)
	}

	// Returns a Number using 52-bits, the max Javascript can use for integer math
	async uint52(): Promise<number> {
		const v = await this.uint64()
		if (v > Number.MAX_SAFE_INTEGER) {
			throw "overflow"
		}

		return Number(v)
	}

	// Returns a Number using 52-bits, the max Javascript can use for integer math
	async vint52(): Promise<number> {
		const v = await this.vint64()
		if (v > Number.MAX_SAFE_INTEGER) {
			throw "overflow"
		}

		return Number(v)
	}

	// NOTE: Returns a BigInt instead of a Number
	async uint64(): Promise<bigint> {
		const view = await this.view(8)
		return view.getBigUint64(0)
	}

	// NOTE: Returns a BigInt instead of a Number
	async vint64(): Promise<bigint> {
		const peek = await this.peek(1)
		const first = new DataView(peek.buffer, peek.byteOffset, peek.byteLength).getUint8(0)
		const size = (first & 0xc0) >> 6

		switch (size) {
		case 0:
			const v0 = await this.uint8()
			return BigInt(v0) & 0x3fn
		case 1:
			const v1 = await this.uint16()
			return BigInt(v1) & 0x3fffn
		case 2:
			const v2 = await this.uint32()
			return BigInt(v2) & 0x3fffffffn
		case 3:
			const v3 = await this.uint64()
			return v3 & 0x3fffffffffffffffn
		default:
			throw "impossible"
		}
	}

	async done(): Promise<boolean> {
		try {
			const peek = await this.peek(1)
			return false
		} catch (err) {
			return true // Assume EOF
		}
	}
}

// StreamWriter wraps a stream and writes chunks of data
export class StreamWriter {
	buffer: ArrayBuffer;
	writer: WritableStreamDefaultWriter;

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
		const v3 = (v) & 0xff

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

		if (v < (1 << 6)) {
			return this.uint8(v)
		} else if (v < (1 << 14)) {
			return this.uint16(v|0x4000)
		} else if (v < (1 << 30)) {
			return this.uint32(v|0x80000000)
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
		if (v < (1 << 6)) {
			return this.uint8(Number(v))
		} else if (v < (1 << 14)) {
			return this.uint16(Number(v)|0x4000)
		} else if (v < (1 << 30)) {
			return this.uint32(Number(v)|0x80000000)
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
