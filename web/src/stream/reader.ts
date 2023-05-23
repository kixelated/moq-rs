// Reader wraps a stream and provides convience methods for reading pieces from a stream
export default class Reader {
	reader: ReadableStream
	buffer: Uint8Array

	constructor(reader: ReadableStream, buffer: Uint8Array = new Uint8Array(0)) {
		this.reader = reader
		this.buffer = buffer
	}

	// Returns any number of bytes
	async read(): Promise<Uint8Array | undefined> {
		if (this.buffer.byteLength) {
			const buffer = this.buffer
			this.buffer = new Uint8Array()
			return buffer
		}

		const r = this.reader.getReader()
		const result = await r.read()

		r.releaseLock()

		return result.value
	}

	async readAll(): Promise<Uint8Array> {
		const r = this.reader.getReader()

		for (;;) {
			const result = await r.read()
			if (result.done) {
				break
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

		const result = this.buffer
		this.buffer = new Uint8Array()

		r.releaseLock()

		return result
	}

	async bytes(size: number): Promise<Uint8Array> {
		const r = this.reader.getReader()

		while (this.buffer.byteLength < size) {
			const result = await r.read()
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

		r.releaseLock()

		return result
	}

	async peek(size: number): Promise<Uint8Array> {
		const r = this.reader.getReader()

		while (this.buffer.byteLength < size) {
			const result = await r.read()
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

		r.releaseLock()

		return result
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
			case 0: {
				const v = await this.uint8()
				return BigInt(v) & 0x3fn
			}
			case 1: {
				const v = await this.uint16()
				return BigInt(v) & 0x3fffn
			}
			case 2: {
				const v = await this.uint32()
				return BigInt(v) & 0x3fffffffn
			}
			case 3: {
				const v = await this.uint64()
				return v & 0x3fffffffffffffffn
			}
			default:
				throw "impossible"
		}
	}

	async done(): Promise<boolean> {
		try {
			await this.peek(1)
			return false
		} catch (err) {
			return true // Assume EOF
		}
	}
}
