import { GroupOrder } from "./group";
import type { Reader, Writer } from "./stream";

export class SubscribeUpdate {
	priority: number;
	order = GroupOrder.Any;

	constructor(priority: number, order: GroupOrder) {
		this.priority = priority;
		this.order = order;
	}

	async encode(w: Writer) {
		await w.u53(this.priority);
		await w.u53(this.order);
	}

	static async decode(r: Reader): Promise<SubscribeUpdate> {
		const priority = await r.u53();
		const order = await r.u53();
		if (order > 2) {
			throw new Error(`invalid order: ${order}`);
		}

		return new SubscribeUpdate(priority, order);
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

	constructor(id: bigint, path: string, priority: number, order: GroupOrder) {
		super(priority, order);
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
		return new Subscribe(id, path, update.priority, update.order);
	}
}

export class SubscribeInfo {
	priority: number;
	order = GroupOrder.Descending;
	latest: number | null;

	constructor(priority: number, order: GroupOrder, latest: number | null) {
		this.priority = priority;
		this.order = order;
		this.latest = latest;
	}

	async encode(w: Writer) {
		await w.u53(this.priority);
		await w.u53(this.order);
		await w.u53(this.latest ? this.latest + 1 : 0);
	}

	static async decode(r: Reader): Promise<SubscribeInfo> {
		const priority = await r.u53();
		const order = await r.u53();
		const latest = await r.u53();
		return new SubscribeInfo(priority, order, latest === 0 ? null : latest - 1);
	}
}
