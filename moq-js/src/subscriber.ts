import { Announced, AnnouncedReader } from "./announced";
import { Group } from "./group";
import { Track, type TrackReader, type TrackWriter } from "./track";
import * as Wire from "./wire";

export class Subscriber {
	#quic: WebTransport;

	// Our subscribed tracks.
	#subscribe = new Map<bigint, TrackWriter>();
	#subscribeNext = 0n;

	// A cache of active subscriptions that can be tee'd.
	#dedupe = new Map<[string, string], TrackReader>();

	constructor(quic: WebTransport) {
		this.#quic = quic;
	}

	announced(prefix = ""): AnnouncedReader {
		const pair = new Announced(prefix);
		const writer = pair.writer;

		const msg = new Wire.AnnounceInterest(prefix);

		(async () => {
			try {
				const stream = await Wire.Stream.open(this.#quic, msg);

				for (;;) {
					const announce = await Wire.Announce.decode_maybe(stream.reader);
					if (!announce) {
						break;
					}

					const broadcast = prefix.concat(announce.suffix);
					await writer.write({ broadcast, active: announce.active });
				}

				await writer.close();
			} catch (err) {
				await writer.abort(err);
			}
		})();

		return pair.reader;
	}

	subscribe(broadcast: string, track: string, priority: number): TrackReader {
		const existing = this.#dedupe.get([broadcast, track]);
		if (existing) {
			// Important that we tee here, so there can be multiple subscribers.
			return existing.tee();
		}

		const id = this.#subscribeNext++;
		const msg = new Wire.Subscribe(id, broadcast, track, priority);

		const pair = new Track(track, priority);
		const reader = pair.reader;
		const writer = pair.writer;

		// Save the writer so we can append groups to it.
		this.#subscribe.set(id, writer);

		// NOTE: We store the non-tee'd reader here so it doesn't count againt the active subscriptions.
		this.#dedupe.set([broadcast, track], pair.reader);

		// Run the subscription in a background promise.
		(async () => {
			const stream = await Wire.Stream.open(this.#quic, msg);
			console.log("opened stream", stream);
			try {
				const info = await Wire.SubscribeOk.decode(stream.reader);
				console.log("subscribe ok", info);

				await stream.reader.closed();
				console.log("waiting for close");
				pair.writer.close();
			} catch (err) {
				console.log("aborting", err);
				pair.writer.abort(err);
			} finally {
				this.#subscribe.delete(id);
				this.#dedupe.delete([broadcast, track]);
			}
		})();

		return reader;
	}

	async runGroup(group: Wire.Group, stream: Wire.Reader) {
		const subscribe = this.#subscribe.get(group.subscribe);
		if (!subscribe) return;

		const pair = new Group(group.sequence);
		subscribe.insertGroup(pair.reader);

		try {
			for (;;) {
				const done = await stream.done();
				if (done) break;

				const size = await stream.u53();
				const payload = await stream.read(size);
				if (!payload) break;

				await pair.writer.writeFrame(payload);
			}

			pair.writer.close();
		} catch (err) {
			pair.writer.abort(err);
		}
	}
}
