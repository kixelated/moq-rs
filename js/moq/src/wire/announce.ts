import type { Reader, Writer } from "./stream";

export class Announce {
	suffix: string;
	active: boolean;

	constructor(suffix: string, active: boolean) {
		this.suffix = suffix;
		this.active = active;
	}

	async encode(w: Writer) {
		await w.u53(this.active ? 1 : 0);
		await w.string(this.suffix);
	}

	static async decode(r: Reader): Promise<Announce> {
		const active = (await r.u53()) === 1;
		const suffix = await r.string();
		return new Announce(suffix, active);
	}

	static async decode_maybe(r: Reader): Promise<Announce | undefined> {
		if (await r.done()) return;
		return await Announce.decode(r);
	}
}

export class AnnounceInterest {
	static StreamID = 0x1;
	prefix: string;

	constructor(prefix: string) {
		this.prefix = prefix;
	}

	async encode(w: Writer) {
		await w.string(this.prefix);
	}

	static async decode(r: Reader): Promise<AnnounceInterest> {
		const prefix = await r.string();
		return new AnnounceInterest(prefix);
	}
}
