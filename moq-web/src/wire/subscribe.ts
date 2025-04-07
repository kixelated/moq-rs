import type { Reader, Writer } from "./stream";

export class SubscribeUpdate {
	priority: number;

	constructor(priority: number) {
		this.priority = priority;
	}

	async encode(w: Writer) {
		await w.u53(this.priority);
	}

	static async decode(r: Reader): Promise<SubscribeUpdate> {
		const priority = await r.u53();
		return new SubscribeUpdate(priority);
	}

	static async decode_maybe(r: Reader): Promise<SubscribeUpdate | undefined> {
		if (await r.done()) return;
		return await SubscribeUpdate.decode(r);
	}
}

export class Subscribe extends SubscribeUpdate {
	id: bigint;
	path: string;

	static StreamID = 0x2;

	constructor(id: bigint, path: string, priority: number) {
		super(priority);
		this.id = id;
		this.path = path;
	}

	override async encode(w: Writer) {
		await w.u62(this.id);
		await w.string(this.path);
		await super.encode(w);
	}

	static override async decode(r: Reader): Promise<Subscribe> {
		const id = await r.u62();
		const path = await r.string();
		const update = await SubscribeUpdate.decode(r);
		return new Subscribe(id, path, update.priority);
	}
}

export class SubscribeInfo {
	priority: number;
	latest: number | null;

	constructor(priority: number, latest: number | null) {
		this.priority = priority;
		this.latest = latest;
	}

	async encode(w: Writer) {
		await w.u53(this.priority);
		await w.u53(this.latest ? this.latest + 1 : 0);
	}

	static async decode(r: Reader): Promise<SubscribeInfo> {
		const priority = await r.u53();
		const latest = await r.u53();
		return new SubscribeInfo(priority, latest === 0 ? null : latest - 1);
	}
}
