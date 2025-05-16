import { Announced } from "./announced";
import { BroadcastConsumer } from "./broadcast";
import type { GroupConsumer } from "./group";
import type { TrackConsumer } from "./track";
import { error } from "./util/error";
import * as Wire from "./wire";

/**
 * Handles publishing broadcasts and managing their lifecycle.
 *
 * @internal
 */
export class Publisher {
	#quic: WebTransport;

	// TODO this will store every announce/unannounce message, which will grow unbounded.
	// We should remove any cached announcements on unannounce, etc.
	#announced = new Announced("");

	// Our published broadcasts.
	#broadcasts = new Map<string, BroadcastConsumer>();

	/**
	 * Creates a new Publisher instance.
	 * @param quic - The WebTransport session to use
	 *
	 * @internal
	 */
	constructor(quic: WebTransport) {
		this.#quic = quic;
	}

	/**
	 * Gets a broadcast reader for the specified broadcast.
	 * @param broadcast - The name of the broadcast to consume
	 * @returns A BroadcastConsumer instance or undefined if not found
	 */
	consume(broadcast: string): BroadcastConsumer | undefined {
		return this.#broadcasts.get(broadcast)?.clone();
	}

	/**
	 * Publishes a broadcast with any associated tracks.
	 * @param broadcast - The broadcast to publish
	 */
	publish(broadcast: BroadcastConsumer) {
		this.#broadcasts.set(broadcast.path, broadcast);
		void this.#runPublish(broadcast);
	}

	async #runPublish(broadcast: BroadcastConsumer) {
		try {
			this.#announced.producer.write({
				broadcast: broadcast.path,
				active: true,
			});

			console.debug(`announce: broadcast=${broadcast.path} active=true`);

			// Wait until the broadcast is closed, then remove it from the lookup.
			await broadcast.closed();

			console.debug(`announce: broadcast=${broadcast.path} active=false`);
		} catch (err: unknown) {
			console.warn(`announce: broadcast=${broadcast.path} error=${error(err)}`);
		} finally {
			broadcast.close();

			this.#broadcasts.delete(broadcast.path);

			this.#announced.producer.write({
				broadcast: broadcast.path,
				active: false,
			});
		}
	}

	/**
	 * Handles an announce interest message.
	 * @param msg - The announce interest message
	 * @param stream - The stream to write announcements to
	 *
	 * @internal
	 */
	async runAnnounce(msg: Wire.AnnounceInterest, stream: Wire.Stream) {
		const reader = this.#announced.consumer.clone();

		for (;;) {
			const announcement = await reader.next();
			if (!announcement) break;

			if (announcement.broadcast.startsWith(msg.prefix)) {
				const suffix = announcement.broadcast.slice(msg.prefix.length);
				const wire = new Wire.Announce(suffix, announcement.active);
				await wire.encode(stream.writer);
			}
		}
	}

	/**
	 * Handles a subscribe message.
	 * @param msg - The subscribe message
	 * @param stream - The stream to write track data to
	 *
	 * @internal
	 */
	async runSubscribe(msg: Wire.Subscribe, stream: Wire.Stream) {
		const broadcast = this.#broadcasts.get(msg.broadcast);
		if (!broadcast) {
			console.debug(`publish unknown: broadcast=${msg.broadcast}`);
			stream.writer.reset(new Error("not found"));
			return;
		}

		const track = broadcast.subscribe(msg.track, msg.priority);
		const serving = this.#runTrack(msg.id, broadcast.path, track, stream.writer);

		for (;;) {
			const decode = Wire.SubscribeUpdate.decode_maybe(stream.reader);

			const result = await Promise.any([serving, decode]);
			if (!result) break;

			if (result instanceof Wire.SubscribeUpdate) {
				// TODO use the update
				console.warn("subscribe update not supported", result);
			}
		}
	}

	/**
	 * Runs a track and sends its data to the stream.
	 * @param sub - The subscription ID
	 * @param broadcast - The broadcast name
	 * @param track - The track to run
	 * @param stream - The stream to write to
	 *
	 * @internal
	 */
	async #runTrack(sub: bigint, broadcast: string, track: TrackConsumer, stream: Wire.Writer) {
		let ok = false;

		try {
			for (;;) {
				const group = await Promise.race([track.nextGroup(), stream.closed()]);
				if (!group) break;

				if (!ok) {
					console.debug(`publish ok: broadcast=${broadcast} track=${track.name}`);

					// Write the SUBSCRIBE_OK message on the first group.
					const info = new Wire.SubscribeOk(track.priority);
					await info.encode(stream);
					ok = true;
				}

				void this.#runGroup(sub, group);
			}

			console.debug(`publish close: broadcast=${broadcast} track=${track.name}`);
			stream.close();
		} catch (err: unknown) {
			const e = error(err);
			console.warn(`publish error: broadcast=${broadcast} track=${track.name} error=${e}`);
			stream.reset(e);
		} finally {
			track.close();
		}
	}

	/**
	 * Runs a group and sends its frames to the stream.
	 * @param sub - The subscription ID
	 * @param group - The group to run
	 *
	 * @internal
	 */
	async #runGroup(sub: bigint, group: GroupConsumer) {
		const msg = new Wire.Group(sub, group.id);
		try {
			const stream = await Wire.Writer.open(this.#quic, msg);
			try {
				for (;;) {
					const frame = await Promise.race([group.readFrame(), stream.closed()]);
					if (!frame) break;

					await stream.u53(frame.byteLength);
					await stream.write(frame);
				}

				stream.close();
			} catch (err: unknown) {
				stream.reset(error(err));
			}
		} finally {
			group.close();
		}
	}
}
