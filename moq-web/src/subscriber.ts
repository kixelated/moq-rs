import { FrameReader } from "./frame";
import type { TrackReader, TrackWriter } from "./track";
import { Queue, Watch } from "./util/async";
import { Closed } from "./util/error";
import * as Wire from "./wire";

export class Subscriber {
	#quic: WebTransport;

	// Our subscribed tracks.
	#subscribe = new Map<bigint, Subscribe>();
	#subscribeNext = 0n;

	constructor(quic: WebTransport) {
		this.#quic = quic;
	}

	async announced(prefix = ""): Promise<Queue<Announced>> {
		const announced = new Queue<Announced>();

		const msg = new Wire.AnnounceInterest(prefix);
		const stream = await Wire.Stream.open(this.#quic, msg);

		this.runAnnounced(stream, announced, prefix)
			.then(() => announced.close())
			.catch((err) => announced.abort(err));

		return announced;
	}

	async runAnnounced(stream: Wire.Stream, announced: Queue<Announced>, prefix: string) {
		const toggle: Map<string, Announced> = new Map();

		try {
			for (;;) {
				const announce = await Wire.Announce.decode_maybe(stream.reader);
				if (!announce) {
					break;
				}

				const path = prefix.concat(announce.suffix);

				const existing = toggle.get(announce.suffix);
				if (existing) {
					if (announce.status === "active") {
						throw new Error("duplicate announce");
					}

					existing.close();
					toggle.delete(announce.suffix);
				} else {
					if (announce.status === "closed") {
						throw new Error("unknown announce");
					}

					const item = new Announced(path);
					await announced.push(item);
					toggle.set(announce.suffix, item);
				}
			}
		} finally {
			for (const item of toggle.values()) {
				item.close();
			}
		}
	}

	// TODO: Deduplicate identical subscribes
	async subscribe(track: TrackWriter): Promise<TrackReader> {
		const id = this.#subscribeNext++;
		const msg = new Wire.Subscribe(id, track.path, track.priority);

		const stream = await Wire.Stream.open(this.#quic, msg);
		const subscribe = new Subscribe(id, stream, track);
		subscribe.run().catch((err) => console.warn("failed to run subscribe: ", err));

		this.#subscribe.set(subscribe.id, subscribe);

		try {
			const info = await Wire.SubscribeInfo.decode(stream.reader);
			console.log("subscribe info", info);

			/*
			for (;;) {
				const dropped = await Wire.GroupDrop.decode(stream.reader)
				console.debug("dropped", dropped)
			}
				*/

			return subscribe.track.reader();
		} catch (err) {
			this.#subscribe.delete(subscribe.id);
			await subscribe.close(Closed.from(err));
			throw err;
		}
	}

	async runGroup(group: Wire.Group, stream: Wire.Reader) {
		const subscribe = this.#subscribe.get(group.subscribe);
		if (!subscribe) return;

		const writer = subscribe.track.createGroup(group.sequence);

		const reader = new FrameReader(stream);
		for (;;) {
			const frame = await reader.read();
			if (!frame) break;

			writer.writeFrame(frame);
		}

		writer.close();
	}
}

export class Announced {
	readonly path: string;

	#closed = new Watch<Closed | undefined>(undefined);

	constructor(path: string) {
		this.path = path;
	}

	close(err = new Closed()) {
		this.#closed.update(err);
	}

	async closed(): Promise<Closed> {
		let [closed, next] = this.#closed.value();
		for (;;) {
			if (closed !== undefined) return closed;
			if (!next) return new Closed();
			[closed, next] = await next;
		}
	}
}

export class Subscribe {
	readonly id: bigint;
	readonly track: TrackWriter;
	readonly stream: Wire.Stream;

	// A queue of received streams for this subscription.
	//#closed = new Watch<Closed | undefined>(undefined);

	constructor(id: bigint, stream: Wire.Stream, track: TrackWriter) {
		this.id = id;
		this.track = track;
		this.stream = stream;
	}

	async run() {
		try {
			await this.closed();
			await this.close();
		} catch (err) {
			await this.close(Closed.from(err));
		}
	}

	async close(closed?: Closed) {
		this.track.close(closed);
		await this.stream.close(closed?.code ?? undefined);
	}

	async closed() {
		await this.stream.reader.closed();
	}
}
