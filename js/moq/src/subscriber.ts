import { AnnouncedConsumer, AnnouncedProducer } from "./announced";
import { BroadcastConsumer, BroadcastProducer } from "./broadcast";
import { GroupProducer } from "./group";
import { type TrackProducer } from "./track";
import { error } from "./util/error";
import * as Wire from "./wire";

/**
 * Handles subscribing to broadcasts and managing their lifecycle.
 *
 * @internal
 */
export class Subscriber {
	#quic: WebTransport;

	// Our subscribed tracks.
	#subscribes = new Map<bigint, TrackProducer>();
	#subscribeNext = 0n;

	// A cache of active subscriptions that can be tee'd.
	#broadcasts = new Map<string, BroadcastConsumer>();

	/**
	 * Creates a new Subscriber instance.
	 * @param quic - The WebTransport session to use
	 *
	 * @internal
	 */
	constructor(quic: WebTransport) {
		this.#quic = quic;
	}

	/**
	 * Gets an announced reader for the specified prefix.
	 * @param prefix - The prefix for announcements
	 * @returns An AnnounceConsumer instance
	 */
	announced(prefix = ""): AnnouncedConsumer {
		const producer = new AnnouncedProducer();
		const consumer = producer.consume(prefix);

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

					const full = prefix.concat(announce.suffix);

					console.debug(`announced: broadcast=${full} active=${announce.active}`);
					producer.write({ path: full, active: announce.active });

					// Just for logging
					if (announce.active) {
						active.add(full);
					} else {
						active.delete(full);
					}
				}

				producer.close();
			} catch (err: unknown) {
				producer.abort(error(err));
			} finally {
				for (const broadcast of active) {
					console.debug(`announced: broadcast=${broadcast} active=dropped`);
				}
			}
		})();

		return consumer;
	}

	/**
	 * Consumes a broadcast from the connection.
	 * @param path - The path of the broadcast to consume
	 * @returns A BroadcastConsumer instance
	 */
	consume(path: string): BroadcastConsumer {
		const existing = this.#broadcasts.get(path);
		if (existing) {
			return existing.clone();
		}

		const producer = new BroadcastProducer();
		const consumer = producer.consume();

		producer.unknownTrack((track) => {
			// Save the track in the cache to deduplicate.
			// NOTE: We don't clone it (yet) so it doesn't count as an active consumer.
			// When we do clone it, we'll only get the most recent (consumed) group.
			producer.insertTrack(track.consume());

			// Perform the subscription in the background.
			this.#runSubscribe(path, track).finally(() => {
				try {
					producer.removeTrack(track.name);
				} catch (err) {
					// Already closed.
					console.warn("track already removed");
				}
			});
		});

		this.#broadcasts.set(path, consumer.clone());

		// Close when the producer has no more consumers.
		producer.unused().finally(() => {
			producer.close();
			this.#broadcasts.delete(path);
		});

		return consumer;
	}

	async #runSubscribe(path: string, track: TrackProducer) {
		const id = this.#subscribeNext++;

		// Save the writer so we can append groups to it.
		this.#subscribes.set(id, track);

		const msg = new Wire.Subscribe(id, path, track.name, track.priority);

		const stream = await Wire.Stream.open(this.#quic, msg);
		try {
			await Wire.SubscribeOk.decode(stream.reader);
			console.debug(`subscribe ok: id=${id} broadcast=${path} track=${track.name}`);

			await Promise.race([stream.reader.closed(), track.unused()]);

			track.close();
			console.debug(`subscribe close: id=${id} broadcast=${path} track=${track.name}`);
		} catch (err) {
			track.abort(error(err));
			console.warn(`subscribe error: id=${id} broadcast=${path} track=${track.name} error=${error(err)}`);
		} finally {
			this.#subscribes.delete(id);
			stream.close();
		}
	}

	/**
	 * Handles a group message.
	 * @param group - The group message
	 * @param stream - The stream to read frames from
	 *
	 * @internal
	 */
	async runGroup(group: Wire.Group, stream: Wire.Reader) {
		const subscribe = this.#subscribes.get(group.subscribe);
		if (!subscribe) {
			console.warn(`unknown subscription: id=${group.subscribe}`);
			return;
		}

		const producer = new GroupProducer(group.sequence);
		subscribe.insertGroup(producer.consume());

		try {
			for (;;) {
				const done = await Promise.race([stream.done(), subscribe.unused(), producer.unused()]);
				if (done !== false) break;

				const size = await stream.u53();
				const payload = await stream.read(size);
				if (!payload) break;

				producer.writeFrame(payload);
			}

			producer.close();
		} catch (err: unknown) {
			producer.abort(error(err));
		}
	}
}
