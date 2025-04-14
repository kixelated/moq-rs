import type { GroupReader } from "./group";
import type { TrackReader, TrackWriter } from "./track";
import { Watch } from "./util/async";
import { Fanout } from "./util/fanout";
import * as Wire from "./wire";

export class Publisher {
	#quic: WebTransport;

	// Our published tracks.
	#tracks = new Watch(new Map<string, TrackReader>());

	// Their subscribed tracks.
	#subscribe = new Map<bigint, Subscribed>();

	constructor(quic: WebTransport) {
		this.#quic = quic;
	}

	// Publish a track
	publish(track: TrackReader) {
		this.#tracks.update((current) => {
			current.set(track.path, track);
			return current;
		});

		track.closed().finally(() =>
			this.#tracks.update((current) => {
				current.delete(track.path);
				return current;
			}),
		);
	}

	#get(path: string): TrackReader | undefined {
		return this.#tracks.latest()[0].get(path);
	}

	// TODO: This is very inefficient for many tracks.
	async runAnnounce(msg: Wire.AnnounceInterest, stream: Wire.Stream) {
		let current = this.#tracks.latest();
		let seen = new Set<string>();

		while (current) {
			const newSeen = new Set<string>();

			for (const [key, announce] of current[0]) {
				if (announce.path.length < msg.prefix.length) {
					continue;
				}

				const prefix = announce.path.slice(0, msg.prefix.length);
				if (prefix !== msg.prefix) {
					continue;
				}

				newSeen.add(key);

				if (seen.delete(key)) {
					// Already exists
					continue;
				}

				// Add a new track
				const suffix = announce.path.slice(msg.prefix.length);
				const active = new Wire.Announce(suffix, "active");
				await active.encode(stream.writer);
			}

			// Remove any closed tracks
			for (const announce of seen) {
				const suffix = announce.slice(msg.prefix.length);
				const ended = new Wire.Announce(suffix, "closed");
				await ended.encode(stream.writer);
			}

			seen = newSeen;

			const next = await current[1];
			if (!next) return;
			current = next;
		}
	}

	async runSubscribe(msg: Wire.Subscribe, stream: Wire.Stream) {
		if (this.#subscribe.has(msg.id)) {
			throw new Error(`duplicate subscribe for id: ${msg.id}`);
		}

		const track = this.#get(msg.path);
		if (!track) {
			await stream.writer.reset(404);
			return;
		}

		const subscribe = new Subscribed(msg, track, this.#quic);

		// TODO close the stream when done
		subscribe.run().catch((err) => console.warn("failed to run subscribe: ", err));

		const info = new Wire.SubscribeOk(track.priority, track.latest);
		await info.encode(stream.writer);

		for (;;) {
			// TODO try_decode
			const update = await Wire.SubscribeUpdate.decode_maybe(stream.reader);
			if (!update) break;

			// TODO use the update
		}
	}
}

class Subscribed {
	#id: bigint;
	#track: TrackReader;
	#quic: WebTransport;

	constructor(msg: Wire.Subscribe, track: TrackReader, quic: WebTransport) {
		this.#id = msg.id;
		this.#track = track;
		this.#quic = quic;
	}

	async run() {
		for (;;) {
			const group = await this.#track.read();
			if (!group) break;

			this.#runGroup(group).catch((err) => console.warn("failed to run group: ", err));
		}
	}

	async #runGroup(group: GroupReader) {
		const msg = new Wire.Group(this.#id, group.id);
		const stream = await Wire.Writer.open(this.#quic, msg);

		for (;;) {
			const frame = await group.nextFrame();
			if (!frame) break;

			await stream.u53(frame.byteLength);
			await stream.write(frame);
		}

		await stream.close();
	}
}
