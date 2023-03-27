import * as Message from "./message"
import * as Stream from "../stream"
import * as MP4 from "../mp4"

import Video from "../video/index"

///<reference path="./types/webtransport.d.ts"/>

export interface PlayerInit {
	url: string;
	canvas: HTMLCanvasElement;
}

export class Player {
	quic: Promise<WebTransport>;
	api: Promise<WritableStream>;
    tracks: Map<string, MP4.InitParser>

	//audio: Worker;
	video: Video;

	constructor(props: PlayerInit) {
		this.tracks = new Map();

		//this.audio = new Worker("../audio")
		this.video = new Video({
			canvas: props.canvas.transferControlToOffscreen(),
		})

		this.quic = this.connect(props.url)

		// Create a unidirectional stream for all of our messages
		this.api = this.quic.then((q) => {
			return q.createUnidirectionalStream()
		})

		// async functions
		this.receiveStreams()
	}

	async close() {
		(await this.quic).close()
	}

	async connect(url: string): Promise<WebTransport> {
		// TODO remove this when WebTransport supports the system CA pool
		const fingerprintURL = new URL(url);
		fingerprintURL.pathname = "/fingerprint"

		const response = await fetch(fingerprintURL)
		if (!response.ok) {
			throw new Error('failed to get server fingerprint');
		}

		const hex = await response.text()

		// Convert the hex to binary.
		let fingerprint = [];
		for (let c = 0; c < hex.length; c += 2) {
			fingerprint.push(parseInt(hex.substring(c, c+2), 16));
		}

		//const fingerprint = Uint8Array.from(atob(hex), c => c.charCodeAt(0))

		const quic = new WebTransport(url, { 
			"serverCertificateHashes": [{
				"algorithm": "sha-256",
				"value": new Uint8Array(fingerprint),
			}]
		})

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

		while (true) {
			const result = await streams.read()
			if (result.done) break

			const stream = result.value
			this.handleStream(stream) // don't await
		}
	}

	async handleStream(stream: ReadableStream) {
		let r = new Stream.Reader(stream)

		while (!await r.done()) {
			const size = await r.uint32();
			const typ = new TextDecoder('utf-8').decode(await r.bytes(4));

			if (typ != "warp") throw "expected warp atom"
			if (size < 8) throw "atom too small"

			const payload = new TextDecoder('utf-8').decode(await r.bytes(size - 8));
			const msg = JSON.parse(payload)

			if (msg.init) {
				return this.handleInit(r, msg.init as Message.Init)
			} else if (msg.segment) {
				return this.handleSegment(r, msg.segment as Message.Segment)
			}
		}
	}

	async handleInit(stream: Stream.Reader, msg: Message.Init) {
        let track = this.tracks.get(msg.id);
        if (!track) {
            track = new MP4.InitParser()
            this.tracks.set(msg.id, track)
        }

        while (1) {
            const data = await stream.read()
            if (!data) break

            track.push(data)
        }

		const info = await track.info

        if (info.audioTracks.length + info.videoTracks.length != 1) {
            throw new Error("expected a single track")
        }

		if (info.videoTracks.length) {
			this.video.init({
				track: msg.id,
				info: info,
				raw: track.raw,
			})
		}
	}

	async handleSegment(stream: Stream.Reader, msg: Message.Segment) {
        let track = this.tracks.get(msg.init);
        if (!track) {
            track = new MP4.InitParser()
            this.tracks.set(msg.init, track)
        }

		const info = await track.info

		// Wait until we learn if this is an audio or video track

		if (info.videoTracks.length) {
			this.video.segment({
				track: msg.init,
				buffer: stream.buffer,
				reader: stream.reader,
			})
		}
	}
}