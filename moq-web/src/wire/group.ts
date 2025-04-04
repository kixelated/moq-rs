import type { Reader, Writer } from "./stream";

export enum GroupOrder {
	Any = 0,
	Ascending = 1,
	Descending = 2,
}

export class Group {
	subscribe: bigint;
	sequence: number;

	static StreamID = 0x0;

	constructor(subscribe: bigint, sequence: number) {
		this.subscribe = subscribe;
		this.sequence = sequence;
	}

	async encode(w: Writer) {
		await w.u62(this.subscribe);
		await w.u53(this.sequence);
	}

	static async decode(r: Reader): Promise<Group> {
		return new Group(await r.u62(), await r.u53());
	}
}

export class GroupDrop {
	sequence: number;
	count: number;
	error: number;

	constructor(sequence: number, count: number, error: number) {
		this.sequence = sequence;
		this.count = count;
		this.error = error;
	}

	async encode(w: Writer) {
		await w.u53(this.sequence);
		await w.u53(this.count);
		await w.u53(this.error);
	}

	static async decode(r: Reader): Promise<GroupDrop> {
		return new GroupDrop(await r.u53(), await r.u53(), await r.u53());
	}
}

export class Frame {
	payload: Uint8Array;

	constructor(payload: Uint8Array) {
		this.payload = payload;
	}

	async encode(w: Writer) {
		await w.u53(this.payload.byteLength);
		await w.write(this.payload);
	}

	static async decode(r: Reader): Promise<Frame> {
		const size = await r.u53();
		const payload = await r.read(size);
		return new Frame(payload);
	}
}
