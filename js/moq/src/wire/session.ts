import type { Reader, Writer } from "./stream";

export const Version = {
	DRAFT_00: 0xff000000,
	DRAFT_01: 0xff000001,
	DRAFT_02: 0xff000002,
	DRAFT_03: 0xff000003,
	FORK_00: 0xff0bad00,
	FORK_01: 0xff0bad01,
	FORK_02: 0xff0bad02,
	FORK_03: 0xff0bad03,
	FORK_04: 0xff0bad04,
} as const;

export class Extensions {
	entries: Map<bigint, Uint8Array>;

	constructor() {
		this.entries = new Map();
	}

	set(id: bigint, value: Uint8Array) {
		this.entries.set(id, value);
	}

	get(id: bigint): Uint8Array | undefined {
		return this.entries.get(id);
	}

	remove(id: bigint): Uint8Array | undefined {
		const value = this.entries.get(id);
		this.entries.delete(id);
		return value;
	}

	async encode(w: Writer) {
		await w.u53(this.entries.size);
		for (const [id, value] of this.entries) {
			await w.u62(id);
			await w.u53(value.length);
			await w.write(value);
		}
	}

	static async decode(r: Reader): Promise<Extensions> {
		const count = await r.u53();
		const params = new Extensions();

		for (let i = 0; i < count; i++) {
			const id = await r.u62();
			const size = await r.u53();
			const value = await r.read(size);

			if (params.entries.has(id)) {
				throw new Error(`duplicate parameter id: ${id}`);
			}

			params.entries.set(id, value);
		}

		return params;
	}
}

export class SessionClient {
	versions: number[];
	extensions: Extensions;

	static StreamID = 0x0;

	constructor(versions: number[], extensions = new Extensions()) {
		this.versions = versions;
		this.extensions = extensions;
	}

	async encode(w: Writer) {
		await w.u53(this.versions.length);
		for (const v of this.versions) {
			await w.u53(v);
		}

		await this.extensions.encode(w);
	}

	static async decode(r: Reader): Promise<SessionClient> {
		const versions: number[] = [];
		const count = await r.u53();
		for (let i = 0; i < count; i++) {
			versions.push(await r.u53());
		}

		const extensions = await Extensions.decode(r);
		return new SessionClient(versions, extensions);
	}
}

export class SessionServer {
	version: number;
	extensions: Extensions;

	constructor(version: number, extensions = new Extensions()) {
		this.version = version;
		this.extensions = extensions;
	}

	async encode(w: Writer) {
		await w.u53(this.version);
		await this.extensions.encode(w);
	}

	static async decode(r: Reader): Promise<SessionServer> {
		const version = await r.u53();
		const extensions = await Extensions.decode(r);
		return new SessionServer(version, extensions);
	}
}

export class SessionInfo {
	bitrate: number;

	constructor(bitrate: number) {
		this.bitrate = bitrate;
	}

	async encode(w: Writer) {
		await w.u53(this.bitrate);
	}

	static async decode(r: Reader): Promise<SessionInfo> {
		const bitrate = await r.u53();
		return new SessionInfo(bitrate);
	}

	static async decode_maybe(r: Reader): Promise<SessionInfo | undefined> {
		if (await r.done()) return;
		return await SessionInfo.decode(r);
	}
}
