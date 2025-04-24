import { AnnouncedReader } from "./announced";
import { Publisher } from "./publisher";
import { Subscriber } from "./subscriber";
import { TrackReader, TrackWriter } from "./track";
import * as Hex from "./util/hex";
import * as Wire from "./wire";

// A pool of connections.
const pool: Map<URL, Promise<Connection>> = new Map();

export class Connection {
	// The URL of the connection.
	#url: URL;

	// The established WebTransport session.
	#quic: WebTransport;

	// Use to receive/send session messages.
	#session: Wire.Stream;

	// Module for contributing tracks.
	#publisher: Publisher;

	// Module for distributing tracks.
	#subscriber: Subscriber;

	// The ref count, closed when it reaches 0.
	#refs = 1;

	constructor(url: URL, quic: WebTransport, session: Wire.Stream) {
		this.#url = url;
		this.#quic = quic;
		this.#session = session;

		this.#publisher = new Publisher(this.#quic);
		this.#subscriber = new Subscriber(this.#quic);

		this.#run().catch((err) => console.error("failed to run connection: ", err));
	}

	// Connect to a server at the given URL.
	// This uses a connection pool under the hood.
	static async connect(url: URL): Promise<Connection> {
		const cached = pool.get(url);
		if (cached) {
			cached.then((conn) => conn.#refs++);
			return cached;
		}

		const connect = Connection.#connect(url);
		pool.set(url, connect);
		return connect;
	}

	static async #connect(url: URL): Promise<Connection> {
		const options: WebTransportOptions = {
			allowPooling: false,
			congestionControl: "low-latency",
			requireUnreliable: true,
		};

		if (url.protocol === "http:") {
			const fingerprintUrl = new URL(url);
			fingerprintUrl.pathname = "/certificate.sha256";

			// Fetch the fingerprint from the server.
			const fingerprint = await fetch(fingerprintUrl);
			const bytes = Hex.decode(await fingerprint.text());

			options.serverCertificateHashes = [
				{
					algorithm: "sha-256",
					value: bytes,
				},
			];

			url.protocol = "https:";
		}

		const quic = new WebTransport(url, options);
		quic.closed.then(() => {
			pool.delete(url);
		});

		await quic.ready;

		const client = new Wire.SessionClient([Wire.Version.FORK_04]);
		const stream = await Wire.Stream.open(quic, client);

		const server = await Wire.SessionServer.decode(stream.reader);
		if (server.version !== Wire.Version.FORK_04) {
			throw new Error(`unsupported server version: ${server.version}`);
		}

		console.log(`established connection: version=${server.version}`);
		return new Connection(url, quic, stream);
	}

	get url(): URL {
		return this.#url;
	}

	close() {
		this.#refs--;

		if (this.#refs <= 0) {
			this.#quic.close();
		}
	}

	async #run(): Promise<void> {
		const session = this.#runSession();
		const bidis = this.#runBidis();
		const unis = this.#runUnis();

		await Promise.all([session, bidis, unis]);
	}

	publish(broadcast: string, track: TrackReader) {
		this.#publisher.publish(broadcast, track);
	}

	announced(prefix = ""): AnnouncedReader {
		return this.#subscriber.announced(prefix);
	}

	subscribe(broadcast: string, track: string, priority = 0): TrackReader {
		return this.#subscriber.subscribe(broadcast, track, priority);
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
				.catch((err) => stream.writer.reset(err))
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
				.catch((err) => stream.stop(err))
				.finally(() => stream.stop(0));
		}
	}

	async #runUni(msg: Wire.StreamUni, stream: Wire.Reader) {
		console.debug("received uni stream: ", msg);

		if (msg instanceof Wire.Group) {
			if (!this.#subscriber) {
				throw new Error("not a subscriber");
			}

			await this.#subscriber.runGroup(msg, stream);
		}
	}

	async closed(): Promise<void> {
		await this.#quic.closed;
	}
}
