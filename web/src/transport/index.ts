import * as Stream from "../stream"
import * as Interface from "./interface"

export interface Config {
	url: string
	fingerprint?: WebTransportHash // the certificate fingerprint, temporarily needed for local development
}

export default class Transport {
	quic: Promise<WebTransport>
	api: Promise<WritableStream>
	callback?: Interface.Callback

	constructor(config: Config) {
		this.quic = this.connect(config)

		// Create a unidirectional stream for all of our messages
		this.api = this.quic.then((q) => {
			return q.createUnidirectionalStream()
		})

		// async functions
		this.receiveStreams()
	}

	async close() {
		;(await this.quic).close()
	}

	// Helper function to make creating a promise easier
	private async connect(config: Config): Promise<WebTransport> {
		const options: WebTransportOptions = {}
		if (config.fingerprint) {
			options.serverCertificateHashes = [config.fingerprint]
		}

		const quic = new WebTransport(config.url, options)
		await quic.ready
		return quic
	}

	async sendMessage(msg: any) {
		const payload = JSON.stringify(msg)
		const size = payload.length + 8

		const stream = await this.api

		const writer = new Stream.Writer(stream)
		await writer.uint32(size)
		await writer.string("warp")
		await writer.string(payload)
		writer.release()
	}

	async receiveStreams() {
		const q = await this.quic
		const streams = q.incomingUnidirectionalStreams.getReader()

		for (;;) {
			const result = await streams.read()
			if (result.done) break

			const stream = result.value
			this.handleStream(stream) // don't await
		}
	}

	async handleStream(stream: ReadableStream) {
		const r = new Stream.Reader(stream)

		while (!(await r.done())) {
			const size = await r.uint32()
			const typ = new TextDecoder("utf-8").decode(await r.bytes(4))

			if (typ != "warp") throw "expected warp atom"
			if (size < 8) throw "atom too small"

			const payload = new TextDecoder("utf-8").decode(await r.bytes(size - 8))
			const msg = JSON.parse(payload)

			if (msg.init) {
				return this.callback?.onInit({
					buffer: r.buffer,
					reader: r.reader,
				})
			} else if (msg.segment) {
				return this.callback?.onSegment({
					buffer: r.buffer,
					reader: r.reader,
				})
			} else {
				console.warn("unknown message", msg)
			}
		}
	}
}
