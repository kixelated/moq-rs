import { Announced, AnnouncedConsumer } from "./announced";
import { Broadcast, BroadcastConsumer } from "./broadcast";
import { Group } from "./group";
import { type TrackProducer } from "./track";
import { error } from "./util/error";
import * as Wire from "./wire";

/**
 * Handles subscribing to broadcasts and managing their lifecycle.
 *
 * @beta
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
	 *
	 * @beta
	 */
	announced(prefix = ""): AnnouncedConsumer {
		const pair = new Announced(prefix);
		const writer = pair.producer;

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
					writer.write({ broadcast, active: announce.active });

					// Just for logging
					if (announce.active) {
						active.add(broadcast);
					} else {
						active.delete(broadcast);
					}
				}

				writer.close();
			} catch (err: unknown) {
				writer.abort(error(err));
			} finally {
				for (const broadcast of active) {
					console.debug(`announced: broadcast=${broadcast} active=dropped`);
				}
			}
		})();

		return pair.consumer;
	}

	/**
	 * Consumes a broadcast from the connection.
	 * @param broadcast - The name of the broadcast to consume
	 * @returns A BroadcastConsumer instance
	 *
	 * @beta
	 */
	consume(broadcast: string): BroadcastConsumer {
		const existing = this.#broadcasts.get(broadcast);
		if (existing) {
			return existing.clone();
		}

		const pair = new Broadcast(broadcast);
		pair.producer.unknownTrack((track) => {
			// Save the track in the cache to deduplicate.
			// NOTE: We don't clone it (yet) so it doesn't count as an active consumer.
			// When we do clone it, we'll only get the most recent (consumed) group.
			pair.producer.insertTrack(track.consumer);

			// Perform the subscription in the background.
			this.#runSubscribe(broadcast, track.producer).finally(() => {
				try {
					pair.producer.removeTrack(track.name);
				} catch (err) {
					// Already closed.
				}
			});
		});

		this.#broadcasts.set(broadcast, pair.consumer);

		// Delete when there are no more consumers.
		pair.consumer.closed().finally(() => {
			this.#broadcasts.delete(broadcast);
		});

		return pair.consumer;
	}

	async #runSubscribe(broadcast: string, track: TrackProducer) {
		const id = this.#subscribeNext++;

		// Save the writer so we can append groups to it.
		this.#subscribes.set(id, track);

		const msg = new Wire.Subscribe(id, broadcast, track.name, track.priority);

		const stream = await Wire.Stream.open(this.#quic, msg);
		try {
			await Wire.SubscribeOk.decode(stream.reader);
			console.debug(`subscribe ok: broadcast=${broadcast} track=${track.name}`);

			await Promise.race([stream.reader.closed(), track.closed()]);

			track.close();
		} catch (err) {
			track.abort(error(err));
		} finally {
			console.debug(`subscribe close: broadcast=${broadcast} track=${track.name}`);
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
		if (!subscribe) return;

		const pair = new Group(group.sequence);
		subscribe.insertGroup(pair.consumer);

		try {
			for (;;) {
				const done = await Promise.race([stream.done(), subscribe.closed(), pair.producer.closed()]);
				if (done !== false) break;

				const size = await stream.u53();
				const payload = await stream.read(size);
				if (!payload) break;

				pair.producer.writeFrame(payload);
			}

			pair.close();
		} catch (err: unknown) {
			pair.abort(error(err));
		}
	}
}
