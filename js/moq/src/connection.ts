import { Buffer } from "buffer";
import { AnnouncedConsumer } from "./announced";
import { BroadcastConsumer } from "./broadcast";
import { Publisher } from "./publisher";
import { Subscriber } from "./subscriber";
import * as Wire from "./wire";

/**
 * Represents a connection to a MoQ server.
 *
 * @public
 */
export class Connection {
	// The URL of the connection.
	readonly url: URL;

	// The established WebTransport session.
	#quic: WebTransport;

	// Use to receive/send session messages.
	#session: Wire.Stream;

	// Module for contributing tracks.
	#publisher: Publisher;

	// Module for distributing tracks.
	#subscriber: Subscriber;

	/**
	 * Creates a new Connection instance.
	 * @param url - The URL of the connection
	 * @param quic - The WebTransport session
	 * @param session - The session stream
	 *
	 * @internal
	 */
	private constructor(url: URL, quic: WebTransport, session: Wire.Stream) {
		this.url = url;
		this.#quic = quic;
		this.#session = session;

		this.#publisher = new Publisher(this.#quic);
		this.#subscriber = new Subscriber(this.#quic);

		this.#run().catch((err: unknown) => {
			console.error("failed to run connection: ", err);
		});
	}

	/**
	 * Establishes a connection to a MOQ server.
	 *
	 * @param url - The URL of the server to connect to
	 * @returns A promise that resolves to a Connection instance
	 */
	static async connect(url: URL): Promise<Connection> {
		const options: WebTransportOptions = {
			allowPooling: false,
			congestionControl: "low-latency",
			requireUnreliable: true,
		};

		let adjustedUrl = url;

		if (url.protocol === "http:") {
			const fingerprintUrl = new URL(url);
			fingerprintUrl.pathname = "/certificate.sha256";

			// Fetch the fingerprint from the server.
			const fingerprint = await fetch(fingerprintUrl);
			const bytes = Buffer.from(await fingerprint.text(), "hex");

			options.serverCertificateHashes = [
				{
					algorithm: "sha-256",
					value: bytes,
				},
			];

			adjustedUrl = new URL(url);
			adjustedUrl.protocol = "https:";
		}

		const quic = new WebTransport(adjustedUrl, options);
		await quic.ready;

		const client = new Wire.SessionClient([Wire.Version.FORK_04]);
		const stream = await Wire.Stream.open(quic, client);

		const server = await Wire.SessionServer.decode(stream.reader);
		if (server.version !== Wire.Version.FORK_04) {
			throw new Error(`unsupported server version: ${server.version.toString()}`);
		}

		const conn = new Connection(adjustedUrl, quic, stream);

		const cleanup = () => {
			conn.close();
		};

		// Attempt to close the connection when the window is closed.
		document.addEventListener("pagehide", cleanup);
		void conn.closed().then(() => {
			document.removeEventListener("pagehide", cleanup);
		});

		return conn;
	}

	/**
	 * Closes the connection.
	 */
	close() {
		try {
			this.#quic.close();
		} catch {
			// ignore
		}
	}

	async #run(): Promise<void> {
		const session = this.#runSession();
		const bidis = this.#runBidis();
		const unis = this.#runUnis();

		await Promise.all([session, bidis, unis]);
	}

	/**
	 * Publishes a broadcast to the connection.
	 * @param broadcast - The broadcast to publish
	 */
	publish(broadcast: BroadcastConsumer) {
		this.#publisher.publish(broadcast);
	}

	/**
	 * Gets an announced reader for the specified prefix.
	 * @param prefix - The prefix for announcements
	 * @returns An AnnounceConsumer instance
	 */
	announced(prefix = ""): AnnouncedConsumer {
		return this.#subscriber.announced(prefix);
	}

	/**
	 * Consumes a broadcast from the connection.
	 *
	 * @remarks
	 * If the broadcast is not found, a "not found" error will be thrown when requesting any tracks.
	 *
	 * @param broadcast - The name of the broadcast to consume
	 * @returns A BroadcastConsumer instance
	 */
	consume(broadcast: string): BroadcastConsumer {
		// To avoid downloading the a broadcast we're publishing, check the publisher first.
		const publisher = this.#publisher.consume(broadcast);
		if (publisher) return publisher;

		return this.#subscriber.consume(broadcast);
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
				.catch((err: unknown) => {
					stream.writer.reset(err);
				})
				.finally(() => {
					stream.writer.close();
				});
		}
	}

	async #runBidi(msg: Wire.StreamBi, stream: Wire.Stream) {
		if (msg instanceof Wire.SessionClient) {
			throw new Error("duplicate session stream");
		}

		if (msg instanceof Wire.AnnounceInterest) {
			await this.#publisher.runAnnounce(msg, stream);
			return;
		}

		if (msg instanceof Wire.Subscribe) {
			await this.#publisher.runSubscribe(msg, stream);
			return;
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
				.then(() => {
					stream.stop(new Error("cancel"));
				})
				.catch((err: unknown) => {
					stream.stop(err);
				});
		}
	}

	async #runUni(msg: Wire.StreamUni, stream: Wire.Reader) {
		if (msg instanceof Wire.Group) {
			await this.#subscriber.runGroup(msg, stream);
		}
	}

	/**
	 * Returns a promise that resolves when the connection is closed.
	 * @returns A promise that resolves when closed
	 */
	async closed(): Promise<void> {
		await this.#quic.closed;
	}
}
