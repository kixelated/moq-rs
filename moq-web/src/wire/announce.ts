import type { Reader, Writer } from "./stream";

export type AnnounceStatus = "active" | "closed";

export class Announce {
	suffix: string;
	status: AnnounceStatus;

	constructor(suffix: string, status: AnnounceStatus) {
		this.suffix = suffix;
		this.status = status;
	}

	async encode(w: Writer) {
		await w.u53(this.status === "active" ? 1 : 0);
		await w.string(this.suffix);
	}

	static async decode(r: Reader): Promise<Announce> {
		const status = (await r.u53()) === 1 ? "active" : "closed";
		const suffix = await r.string();
		return new Announce(suffix, status);
	}

	static async decode_maybe(r: Reader): Promise<Announce | undefined> {
		if (await r.done()) return;
		return await Announce.decode(r);
	}
}

export class AnnounceInterest {
	static StreamID = 0x1;

	constructor(public prefix: string) {}

	async encode(w: Writer) {
		await w.string(this.prefix);
	}

	static async decode(r: Reader): Promise<AnnounceInterest> {
		const prefix = await r.string();
		return new AnnounceInterest(prefix);
	}
}
