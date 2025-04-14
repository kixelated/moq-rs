import { FrameReader } from "./frame";
import type { TrackReader, TrackWriter } from "./track";
import { Queue, Watch } from "./util/async";
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
	}

	// TODO: Deduplicate identical subscribes
	async subscribe(track: TrackWriter): Promise<TrackReader> {
		const id = this.#subscribeNext++;
		const msg = new Wire.Subscribe(id, track.path, track.priority);

		const stream = await Wire.Stream.open(this.#quic, msg);
		const subscribe = new Subscribe(id, stream, track);

		this.#subscribe.set(subscribe.id, subscribe);

		try {
			const info = await Wire.SubscribeOk.decode(stream.reader);
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
			await subscribe.close(err);
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

	constructor(path: string) {
		this.path = path;
	}
}

export class Subscribe {
	readonly id: bigint;
	readonly track: TrackWriter;
	readonly stream: Wire.Stream;

	constructor(id: bigint, stream: Wire.Stream, track: TrackWriter) {
		this.id = id;
		this.track = track;
		this.stream = stream;
	}

	async close(reason?: unknown) {
		await this.stream.close(reason);
	}

	async closed() {
		await this.stream.reader.closed();
	}
}
