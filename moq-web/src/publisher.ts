import type { GroupReader } from "./group";
import type { TrackReader } from "./track";
import { Watch } from "./util/async";
import { Closed } from "./util/error";
import * as Wire from "./wire";

export class Publisher {
	#quic: WebTransport;

	// Our announced tracks.
	#announce = new Watch(new Map<string, TrackReader>());

	// Their subscribed tracks.
	#subscribe = new Map<bigint, Subscribed>();

	constructor(quic: WebTransport) {
		this.#quic = quic;
	}

	// Publish a track
	publish(track: TrackReader) {
		this.#announce.update((current) => {
			current.set(track.path, track);
			return current;
		});

		// TODO: clean up announcements
		// track.closed().then(() => this.#announce.delete(track.path))
	}

	#get(path: string): TrackReader | undefined {
		return this.#announce.value()[0].get(path);
	}

	async runAnnounce(msg: Wire.AnnounceInterest, stream: Wire.Stream) {
		let current = this.#announce.value();
		let seen = new Map<string, TrackReader>();

		while (current) {
			const newSeen = new Map<string, TrackReader>();

			for (const [key, announce] of current[0]) {
				if (announce.path.length < msg.prefix.length) {
					continue;
				}

				const prefix = announce.path.slice(0, msg.prefix.length);
				if (prefix !== msg.prefix) {
					continue;
				}

				newSeen.set(key, announce);

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
			for (const announce of seen.values()) {
				const suffix = announce.path.slice(msg.prefix.length);
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

		try {
			const info = new Wire.SubscribeInfo(track.priority, track.order, track.latest);
			await info.encode(stream.writer);

			for (;;) {
				// TODO try_decode
				const update = await Wire.SubscribeUpdate.decode_maybe(stream.reader);
				if (!update) {
					subscribe.close();
					break;
				}

				// TODO use the update
			}
		} catch (err) {
			subscribe.close(Closed.from(err));
		}
	}
}

class Subscribed {
	#id: bigint;
	#track: TrackReader;
	#quic: WebTransport;

	#closed = new Watch<Closed | undefined>(undefined);

	constructor(msg: Wire.Subscribe, track: TrackReader, quic: WebTransport) {
		this.#id = msg.id;
		this.#track = track;
		this.#quic = quic;
	}

	async run() {
		const closed = this.closed();

		for (;;) {
			const group = await Promise.any([this.#track.nextGroup(), closed]);
			if (group instanceof Closed) {
				this.close(group);
				return;
			}

			if (!group) break;

			this.#runGroup(group).catch((err) => console.warn("failed to run group: ", err));
		}

		// TODO wait until all groups are done
		this.close();
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

	close(err = new Closed()) {
		this.#closed.update(err);
		this.#track.close();
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
