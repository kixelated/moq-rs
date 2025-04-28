import { Announced, AnnouncedReader } from "./announced";
import { Broadcast, BroadcastReader } from "./broadcast";
import { Group } from "./group";
import { Track, type TrackReader, type TrackWriter } from "./track";
import * as Wire from "./wire";

export class Subscriber {
	#quic: WebTransport;

	// Our subscribed tracks.
	#subscribes = new Map<bigint, TrackWriter>();
	#subscribeNext = 0n;

	// A cache of active subscriptions that can be tee'd.
	#broadcasts = new Map<string, BroadcastReader>();

	constructor(quic: WebTransport) {
		this.#quic = quic;
	}

	announced(prefix = ""): AnnouncedReader {
		const pair = new Announced(prefix);
		const writer = pair.writer;

		const msg = new Wire.AnnounceInterest(prefix);

		(async () => {
			const active = new Set<string>();

			try {
				const stream = await Wire.Stream.open(this.#quic, msg);

				for (;;) {
					const announce = await Wire.Announce.decode_maybe(stream.reader);
					if (!announce) {
						break;
					}

					const broadcast = prefix.concat(announce.suffix);

					console.debug(`announced: broadcast=${broadcast} active=${announce.active}`);
					await writer.write({ broadcast, active: announce.active });

					// Just for logging
					if (announce.active) {
						active.add(broadcast);
					} else {
						active.delete(broadcast);
					}
				}

				await writer.close();
			} catch (err) {
				await writer.abort(err);
			} finally {
				for (const broadcast of active) {
					console.debug(`announced: broadcast=${broadcast} active=dropped`);
				}
			}
		})();

		return pair.reader;
	}

	consume(broadcast: string): BroadcastReader {
		const existing = this.#broadcasts.get(broadcast);
		if (existing) {
			return existing.clone();
		}

		const pair = new Broadcast(broadcast);
		pair.writer.unknown((track) => {
			// Save the track in the cache to deduplicate.
			// NOTE: We don't clone it (yet) so it doesn't count as an active consumer.
			// When we do clone it, we'll only get the most recent (consumed) group.
			pair.writer.insert(track.reader);

			// Perform the subscription in the background.
			this.#runSubscribe(broadcast, track.writer).finally(() => {
				pair.writer.remove(track.name);
			});
		});

		this.#broadcasts.set(broadcast, pair.reader);

		// Delete when there are no more consumers.
		pair.writer.closed().finally(() => {
			this.#broadcasts.delete(broadcast);
		});

		return pair.reader;
	}

	async #runSubscribe(broadcast: string, track: TrackWriter) {
		const id = this.#subscribeNext++;

		// Save the writer so we can append groups to it.
		this.#subscribes.set(id, track);

		const msg = new Wire.Subscribe(id, broadcast, track.name, track.priority);

		const stream = await Wire.Stream.open(this.#quic, msg);
		try {
			const _info = await Wire.SubscribeOk.decode(stream.reader);
			console.debug(`subscribe ok: broadcast=${broadcast} track=${track.name}`);

			await Promise.race([stream.reader.closed(), track.closed()]);

			track.close();
		} catch (err) {
			track.abort(err);
		} finally {
			console.debug(`subscribe close: broadcast=${broadcast} track=${track.name}`);
			this.#subscribes.delete(id);
			stream.close();
		}
	}

	async runGroup(group: Wire.Group, stream: Wire.Reader) {
		const subscribe = this.#subscribes.get(group.subscribe);
		if (!subscribe) return;

		const pair = new Group(group.sequence);

		// NOTE: To make sure new consumers get all frames, we clone here.
		// We don't read from our clone so
		await subscribe.insertGroup(pair.reader);

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
