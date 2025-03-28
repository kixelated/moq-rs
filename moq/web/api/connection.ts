import { asError } from "../util/error";

import type { Queue } from "../util/async";
import { Closed } from "../util/error";
import * as Wire from "../wire";
import { Publisher } from "./publisher";
import { type Announced, Subscriber } from "./subscriber";
import type { Track, TrackReader } from "./track";

export class Connection {
	// The established WebTransport session.
	#quic: WebTransport;

	// Use to receive/send session messages.
	#session: Wire.Stream;

	// Module for contributing tracks.
	#publisher: Publisher;

	// Module for distributing tracks.
	#subscriber: Subscriber;

	// Async work running in the background
	#running: Promise<void>;

	constructor(quic: WebTransport, session: Wire.Stream) {
		this.#quic = quic;
		this.#session = session;

		this.#publisher = new Publisher(this.#quic);
		this.#subscriber = new Subscriber(this.#quic);

		this.#running = this.#run();
	}

	close(code = 0, reason = "") {
		this.#quic.close({ closeCode: code, reason });
	}

	async #run(): Promise<void> {
		const session = this.#runSession().catch((err) => new Error("failed to run session: ", err));
		const bidis = this.#runBidis().catch((err) => new Error("failed to run bidis: ", err));
		const unis = this.#runUnis().catch((err) => new Error("failed to run unis: ", err));

		await Promise.all([session, bidis, unis]);
	}

	publish(track: TrackReader) {
		this.#publisher.publish(track);
	}

	async announced(prefix = ""): Promise<Queue<Announced>> {
		return this.#subscriber.announced(prefix);
	}

	async subscribe(track: Track): Promise<TrackReader> {
		return await this.#subscriber.subscribe(track);
	}

	async #runSession() {
		// Receive messages until the connection is closed.
		for (;;) {
			const msg = await Wire.SessionInfo.decode_maybe(this.#session.reader);
			if (!msg) break;
			// TODO use the session info
		}
	}

	async #runBidis() {
		for (;;) {
			const next = await Wire.Stream.accept(this.#quic);
			if (!next) {
				break;
			}

			const [msg, stream] = next;
			this.#runBidi(msg, stream)
				.catch((err) => stream.writer.reset(Closed.extract(err)))
				.finally(() => stream.writer.close());
		}
	}

	async #runBidi(msg: Wire.StreamBi, stream: Wire.Stream) {
		console.debug("received bi stream: ", msg);

		if (msg instanceof Wire.SessionClient) {
			throw new Error("duplicate session stream");
		}

		if (msg instanceof Wire.AnnounceInterest) {
			if (!this.#subscriber) {
				throw new Error("not a subscriber");
			}

			return await this.#publisher.runAnnounce(msg, stream);
		}

		if (msg instanceof Wire.Subscribe) {
			if (!this.#publisher) {
				throw new Error("not a publisher");
			}

			return await this.#publisher.runSubscribe(msg, stream);
		}

		throw new Error("unknown message: ", msg);
	}

	async #runUnis() {
		for (;;) {
			const next = await Wire.Reader.accept(this.#quic);
			if (!next) {
				break;
			}

			const [msg, stream] = next;
			this.#runUni(msg, stream)
				.catch((err) => stream.stop(Closed.extract(err)))
				.finally(() => stream.stop(0));
		}
	}

	async #runUni(msg: Wire.StreamUni, stream: Wire.Reader) {
		console.debug("received uni stream: ", msg);

		if (msg instanceof Wire.Group) {
			if (!this.#subscriber) {
				throw new Error("not a subscriber");
			}

			return this.#subscriber.runGroup(msg, stream);
		}
	}

	async closed(): Promise<Error> {
		try {
			await this.#running;
			return new Error("closed");
		} catch (e) {
			return asError(e);
		}
	}
}
