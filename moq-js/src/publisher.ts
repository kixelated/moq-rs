import { Announced } from "./announced";
import { BroadcastReader } from "./broadcast";
import type { GroupReader } from "./group";
import type { TrackReader, TrackWriter } from "./track";
import * as Wire from "./wire";

export class Publisher {
	#quic: WebTransport;

	// TODO this will store every announce/unannounce message, which will grow unbounded.
	// We should remove any cached announcements on unannounce, etc.
	#announced = new Announced("");

	// Our published broadcasts.
	#broadcasts = new Map<string, BroadcastReader>();

	constructor(quic: WebTransport) {
		this.#quic = quic;
	}

	// Publish a broadcast with any associated tracks.
	publish(broadcast: BroadcastReader) {
		this.#broadcasts.set(broadcast.path, broadcast);

		(async () => {
			try {
				await this.#announced.writer.write({ broadcast: broadcast.path, active: true });
				console.debug(`announce: broadcast=${broadcast.path} active=true`);

				// TODO wait until the broadcast is closed, then remove it from the lookup.
				await broadcast.closed();
			} finally {
				console.debug(`announce: broadcast=${broadcast.path} active=false`);
				this.#broadcasts.delete(broadcast.path);
				await this.#announced.writer.write({ broadcast: broadcast.path, active: false });
			}
		})();
	}

	async runAnnounce(msg: Wire.AnnounceInterest, stream: Wire.Stream) {
		const reader = await this.#announced.reader.clone();

		for (;;) {
			const announcement = await reader.read();
			if (!announcement) break;

			if (announcement.broadcast.startsWith(msg.prefix)) {
				const wire = new Wire.Announce(announcement.broadcast.slice(msg.prefix.length), announcement.active);
				await wire.encode(stream.writer);
			}
		}
	}

	async runSubscribe(msg: Wire.Subscribe, stream: Wire.Stream) {
		const broadcast = this.#broadcasts.get(msg.broadcast);
		if (!broadcast) {
			console.debug(`publish unknown: broadcast=${msg.broadcast}`);
			stream.writer.reset(404);
			return;
		}

		const track = broadcast.subscribe(msg.track, msg.priority);
		const serving = this.#runTrack(msg.id, broadcast.path, track.clone(), stream.writer);

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

	async #runTrack(sub: bigint, broadcast: string, track: TrackReader, stream: Wire.Writer) {
		let ok = false;

		try {
			for (;;) {
				const group = await track.nextGroup();
				if (!group) break;

				if (!ok) {
					console.debug(`publish ok: broadcast=${broadcast} track=${track.name}`);

					// Write the SUBSCRIBE_OK message on the first group.
					const info = new Wire.SubscribeOk(track.priority);
					await info.encode(stream);
					ok = true;
				}

				this.#runGroup(sub, group.clone());
			}

			console.debug(`publish close: broadcast=${broadcast} track=${track.name}`);
			stream.close();
		} catch (err) {
			console.debug(`publish error: broadcast=${broadcast} track=${track.name} error=${err}`);
			stream.reset(err);
		} finally {
			track.close();
		}
	}

	async #runGroup(sub: bigint, group: GroupReader) {
		const msg = new Wire.Group(sub, group.id);
		const stream = await Wire.Writer.open(this.#quic, msg);
		try {
			for (;;) {
				const frame = await group.readFrame();
				if (!frame) break;

				await stream.u53(frame.byteLength);
				await stream.write(frame);
			}

			await stream.close();
		} catch (err) {
			await stream.reset(err);
		} finally {
			group.close();
		}
	}
}
