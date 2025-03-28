import * as Hex from "../util/hex";
import { Connection } from "./connection";
import { Stream } from "../wire/stream";
import { SessionClient, SessionServer, Version } from "../wire/session";

export interface ClientConfig {
	url: string;

	// If set, the server fingerprint will be fetched from this URL.
	// This is required to use self-signed certificates with Chrome (May 2023)
	fingerprint?: string;
}

export class Client {
	#fingerprint: Promise<WebTransportHash | undefined>;

	readonly config: ClientConfig;

	constructor(config: ClientConfig) {
		this.config = config;

		this.#fingerprint = this.#fetchFingerprint(config.fingerprint).catch((e) => {
			console.warn("failed to fetch fingerprint: ", e);
			return undefined;
		});
	}

	async connect(): Promise<Connection> {
		// Helper function to make creating a promise easier
		const options: WebTransportOptions = {};

		const fingerprint = await this.#fingerprint;
		if (fingerprint) options.serverCertificateHashes = [fingerprint];

		const quic = new WebTransport(this.config.url, options);
		await quic.ready;

		const client = new SessionClient([Version.FORK_04]);
		const stream = await Stream.open(quic, client);

		const server = await SessionServer.decode(stream.reader);
		if (server.version !== Version.FORK_04) {
			throw new Error(`unsupported server version: ${server.version}`);
		}

		console.log(`established connection: version=${server.version}`);

		return new Connection(quic, stream);
	}

	async #fetchFingerprint(url?: string): Promise<WebTransportHash | undefined> {
		if (!url) return;

		// TODO remove this fingerprint when Chrome WebTransport accepts the system CA
		const response = await fetch(url);
		const bytes = Hex.decode(await response.text());

		return {
			algorithm: "sha-256",
			value: bytes,
		};
	}
}
