import { Announced } from "./announced";
import type { GroupReader } from "./group";
import type { TrackReader, TrackWriter } from "./track";
import { Watch } from "./util/async";
import * as Wire from "./wire";

export class Publisher {
	#quic: WebTransport;

	// TODO this will store every announce/unannounce message, which will grow unbounded.
	// We should remove any cached announcements on unannounce, etc.
	#announced = new Announced("");

	// Our published tracks.
	#tracks = new Map<string, TrackReader>();

	constructor(quic: WebTransport) {
		this.#quic = quic;
	}

	// Publish a track
	publish(broadcast: string, track: TrackReader) {
		this.#tracks.set(track.name, track);

		(async () => {
			try {
				await this.#announced.writer.write({ broadcast, active: true });

				// Consume the track until it's closed, then remove it from the lookup.
				// This avoids backpressure on the TrackWriter.
				for (;;) {
					const group = await track.next();
					if (!group) break;
				}
			} finally {
				this.#tracks.delete(track.name);
				await this.#announced.writer.write({ broadcast, active: false });
			}
		})();
	}

	async runAnnounce(msg: Wire.AnnounceInterest, stream: Wire.Stream) {
		const reader = await this.#announced.reader.tee();

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
		const track = this.#tracks.get(msg.track);
		if (!track) {
			await stream.writer.reset(404);
			return;
		}

		const info = new Wire.SubscribeOk(track.priority);
		await info.encode(stream.writer);

		const serving = this.#runTrack(msg.id, track.tee());

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

	async #runTrack(sub: bigint, track: TrackReader) {
		try {
			for (;;) {
				const group = await track.next();
				if (!group) break;

				this.#runGroup(sub, group.tee());
			}
		} finally {
			await track.close();
		}
	}

	async #runGroup(sub: bigint, group: GroupReader) {
		const frames = group.frames.getReader();

		const gmsg = new Wire.Group(sub, group.id);
		const stream = await Wire.Writer.open(this.#quic, gmsg);
		try {
			for (;;) {
				const { done, value } = await frames.read();
				if (done) break;

				await stream.u53(value.byteLength);
				await stream.write(value);
			}

			await stream.close();
		} catch (err) {
			await stream.reset(err);
		} finally {
			await frames.cancel();
		}
	}
}
