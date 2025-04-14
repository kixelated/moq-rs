import * as Wire from ".";

const MAX_U6 = 2 ** 6 - 1;
const MAX_U14 = 2 ** 14 - 1;
const MAX_U30 = 2 ** 30 - 1;
const MAX_U31 = 2 ** 31 - 1;
const MAX_U53 = Number.MAX_SAFE_INTEGER;
const MAX_U62: bigint = 2n ** 62n - 1n;

export type StreamBi = Wire.SessionClient | Wire.AnnounceInterest | Wire.Subscribe;
export type StreamUni = Wire.Group;

export class Stream {
	reader: Reader;
	writer: Writer;

	constructor(props: {
		writable: WritableStream<Uint8Array>;
		readable: ReadableStream<Uint8Array>;
	}) {
		this.writer = new Writer(props.writable);
		this.reader = new Reader(props.readable);
	}

	static async accept(quic: WebTransport): Promise<[StreamBi, Stream] | undefined> {
		const reader = quic.incomingBidirectionalStreams.getReader();
		const next = await reader.read();
		reader.releaseLock();

		if (next.done) return;
		const stream = new Stream(next.value);
		let msg: StreamBi;

		const typ = await stream.reader.u8();
		if (typ === Wire.SessionClient.StreamID) {
			msg = await Wire.SessionClient.decode(stream.reader);
		} else if (typ === Wire.AnnounceInterest.StreamID) {
			msg = await Wire.AnnounceInterest.decode(stream.reader);
		} else if (typ === Wire.Subscribe.StreamID) {
			msg = await Wire.Subscribe.decode(stream.reader);
		} else {
			throw new Error(`unknown stream type: ${typ}`);
		}

		console.debug("accepted stream", msg);

		return [msg, stream];
	}

	static async open(quic: WebTransport, msg: StreamBi): Promise<Stream> {
		const stream = new Stream(await quic.createBidirectionalStream());

		if (msg instanceof Wire.SessionClient) {
			await stream.writer.u8(Wire.SessionClient.StreamID);
		} else if (msg instanceof Wire.AnnounceInterest) {
			await stream.writer.u8(Wire.AnnounceInterest.StreamID);
		} else if (msg instanceof Wire.Subscribe) {
			await stream.writer.u8(Wire.Subscribe.StreamID);
		} else {
			// Make sure we're not missing any types.
			msg as never;
			throw new Error("invalid message type");
		}

		await msg.encode(stream.writer);

		console.debug("opened stream", msg);

		return stream;
	}

	async close(reason?: unknown) {
		if (reason === undefined) {
			await this.writer.close();
		} else {
			await this.writer.reset(reason);
			await this.reader.stop(reason);
		}
	}
}

// Reader wraps a stream and provides convience methods for reading pieces from a stream
// Unfortunately we can't use a BYOB reader because it's not supported with WebTransport+WebWorkers yet.
export class Reader {
	#buffer: Uint8Array;
	#stream: ReadableStream<Uint8Array>;
	#reader: ReadableStreamDefaultReader<Uint8Array>;

	constructor(stream: ReadableStream<Uint8Array>, buffer = new Uint8Array()) {
		this.#buffer = buffer;
		this.#stream = stream;
		this.#reader = this.#stream.getReader();
	}

	// Adds more data to the buffer, returning true if more data was added.
	async #fill(): Promise<boolean> {
		const result = await this.#reader.read();
		if (result.done) {
			return false;
		}

		const buffer = new Uint8Array(result.value);

		if (this.#buffer.byteLength === 0) {
			this.#buffer = buffer;
		} else {
			const temp = new Uint8Array(this.#buffer.byteLength + buffer.byteLength);
			temp.set(this.#buffer);
			temp.set(buffer, this.#buffer.byteLength);
			this.#buffer = temp;
		}

		return true;
	}

	// Add more data to the buffer until it's at least size bytes.
	async #fillTo(size: number) {
		while (this.#buffer.byteLength < size) {
			if (!(await this.#fill())) {
				throw new Error("unexpected end of stream");
			}
		}
	}

	// Consumes the first size bytes of the buffer.
	#slice(size: number): Uint8Array {
		const result = new Uint8Array(this.#buffer.buffer, this.#buffer.byteOffset, size);
		this.#buffer = new Uint8Array(this.#buffer.buffer, this.#buffer.byteOffset + size);

		return result;
	}

	async read(size: number): Promise<Uint8Array> {
		if (size === 0) return new Uint8Array();

		await this.#fillTo(size);
		return this.#slice(size);
	}

	async readAll(): Promise<Uint8Array> {
		while (await this.#fill()) {}
		return this.#slice(this.#buffer.byteLength);
	}

	async string(maxLength?: number): Promise<string> {
		const length = await this.u53();
		if (maxLength !== undefined && length > maxLength) {
			throw new Error(`string length ${length} exceeds max length ${maxLength}`);
		}

		const buffer = await this.read(length);
		return new TextDecoder().decode(buffer);
	}

	async u8(): Promise<number> {
		await this.#fillTo(1);
		return this.#slice(1)[0];
	}

	// Returns a Number using 53-bits, the max Javascript can use for integer math
	async u53(): Promise<number> {
		const v = await this.u62();
		if (v > MAX_U53) {
			throw new Error("value larger than 53-bits; use v62 instead");
		}

		return Number(v);
	}

	// NOTE: Returns a bigint instead of a number since it may be larger than 53-bits
	async u62(): Promise<bigint> {
		await this.#fillTo(1);
		const size = (this.#buffer[0] & 0xc0) >> 6;

		if (size === 0) {
			const first = this.#slice(1)[0];
			return BigInt(first) & 0x3fn;
		}
		if (size === 1) {
			await this.#fillTo(2);
			const slice = this.#slice(2);
			const view = new DataView(slice.buffer, slice.byteOffset, slice.byteLength);

			return BigInt(view.getInt16(0)) & 0x3fffn;
		}
		if (size === 2) {
			await this.#fillTo(4);
			const slice = this.#slice(4);
			const view = new DataView(slice.buffer, slice.byteOffset, slice.byteLength);

			return BigInt(view.getUint32(0)) & 0x3fffffffn;
		}
		await this.#fillTo(8);
		const slice = this.#slice(8);
		const view = new DataView(slice.buffer, slice.byteOffset, slice.byteLength);

		return view.getBigUint64(0) & 0x3fffffffffffffffn;
	}

	async done(): Promise<boolean> {
		if (this.#buffer.byteLength > 0) return false;
		return !(await this.#fill());
	}

	async stop(reason?: unknown) {
		this.#reader.releaseLock();
		await this.#stream.cancel(reason);
	}

	async closed() {
		return this.#reader.closed;
	}

	release(): [Uint8Array, ReadableStream<Uint8Array>] {
		this.#reader.releaseLock();
		return [this.#buffer, this.#stream];
	}

	static async accept(quic: WebTransport): Promise<[StreamUni, Reader] | undefined> {
		const reader = quic.incomingUnidirectionalStreams.getReader();
		const next = await reader.read();
		reader.releaseLock();

		if (next.done) return;
		const stream = new Reader(next.value);
		let msg: StreamUni;

		const typ = await stream.u8();
		if (typ === Wire.Group.StreamID) {
			msg = await Wire.Group.decode(stream);
		} else {
			throw new Error(`unknown stream type: ${typ}`);
		}

		return [msg, stream];
	}
}

// Writer wraps a stream and writes chunks of data
export class Writer {
	#scratch: Uint8Array;
	#writer: WritableStreamDefaultWriter<Uint8Array>;
	#stream: WritableStream<Uint8Array>;

	constructor(stream: WritableStream<Uint8Array>) {
		this.#stream = stream;
		this.#scratch = new Uint8Array(8);
		this.#writer = this.#stream.getWriter();
	}

	async u8(v: number) {
		await this.write(setUint8(this.#scratch, v));
	}

	async i32(v: number) {
		if (Math.abs(v) > MAX_U31) {
			throw new Error(`overflow, value larger than 32-bits: ${v}`);
		}

		// We don't use a VarInt, so it always takes 4 bytes.
		// This could be improved but nothing is standardized yet.
		await this.write(setInt32(this.#scratch, v));
	}

	async u53(v: number) {
		if (v < 0) {
			throw new Error(`underflow, value is negative: ${v}`);
		}
		if (v > MAX_U53) {
			throw new Error(`overflow, value larger than 53-bits: ${v}`);
		}

		await this.write(setVint53(this.#scratch, v));
	}

	async u62(v: bigint) {
		if (v < 0) {
			throw new Error(`underflow, value is negative: ${v}`);
		}
		if (v >= MAX_U62) {
			throw new Error(`overflow, value larger than 62-bits: ${v}`);
		}

		await this.write(setVint62(this.#scratch, v));
	}

	async write(v: Uint8Array) {
		await this.#writer.write(v);
	}

	async string(str: string) {
		const data = new TextEncoder().encode(str);
		await this.u53(data.byteLength);
		await this.write(data);
	}

	async close() {
		this.#writer.releaseLock();
		await this.#stream.close();
	}

	async reset(reason?: unknown) {
		this.#writer.releaseLock();
		await this.#stream.abort(reason);
	}

	release(): WritableStream<Uint8Array> {
		this.#writer.releaseLock();
		return this.#stream;
	}

	static async open(quic: WebTransport, msg: StreamUni): Promise<Writer> {
		const stream = new Writer(await quic.createUnidirectionalStream());

		if (msg instanceof Wire.Group) {
			await stream.u8(Wire.Group.StreamID);
		} else {
			// Make sure we're not missing any types.
			msg as never;
			throw new Error("invalid message type");
		}

		await msg.encode(stream);

		return stream;
	}
}

export function setUint8(dst: Uint8Array, v: number): Uint8Array {
	dst[0] = v;
	return dst.slice(0, 1);
}

export function setUint16(dst: Uint8Array, v: number): Uint8Array {
	const view = new DataView(dst.buffer, dst.byteOffset, 2);
	view.setUint16(0, v);

	return new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
}

export function setInt32(dst: Uint8Array, v: number): Uint8Array {
	const view = new DataView(dst.buffer, dst.byteOffset, 4);
	view.setInt32(0, v);

	return new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
}

export function setUint32(dst: Uint8Array, v: number): Uint8Array {
	const view = new DataView(dst.buffer, dst.byteOffset, 4);
	view.setUint32(0, v);

	return new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
}

export function setVint53(dst: Uint8Array, v: number): Uint8Array {
	if (v <= MAX_U6) {
		return setUint8(dst, v);
	}
	if (v <= MAX_U14) {
		return setUint16(dst, v | 0x4000);
	}
	if (v <= MAX_U30) {
		return setUint32(dst, v | 0x80000000);
	}
	if (v <= MAX_U53) {
		return setUint64(dst, BigInt(v) | 0xc000000000000000n);
	}
	throw new Error(`overflow, value larger than 53-bits: ${v}`);
}

export function setVint62(dst: Uint8Array, v: bigint): Uint8Array {
	if (v < MAX_U6) {
		return setUint8(dst, Number(v));
	}
	if (v < MAX_U14) {
		return setUint16(dst, Number(v) | 0x4000);
	}
	if (v <= MAX_U30) {
		return setUint32(dst, Number(v) | 0x80000000);
	}
	if (v <= MAX_U62) {
		return setUint64(dst, BigInt(v) | 0xc000000000000000n);
	}
	throw new Error(`overflow, value larger than 62-bits: ${v}`);
}

export function setUint64(dst: Uint8Array, v: bigint): Uint8Array {
	const view = new DataView(dst.buffer, dst.byteOffset, 8);
	view.setBigUint64(0, v);

	return new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
}
