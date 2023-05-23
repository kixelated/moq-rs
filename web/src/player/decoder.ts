import * as Message from "./message"
import * as MP4 from "../mp4"
import * as Stream from "../stream"

import Renderer from "./renderer"

export default class Decoder {
	init: MP4.InitParser
	decoders: Map<number, AudioDecoder | VideoDecoder>
	renderer: Renderer

	constructor(renderer: Renderer) {
		this.init = new MP4.InitParser()
		this.decoders = new Map()
		this.renderer = renderer
	}

	async receiveInit(msg: Message.Init) {
		const stream = new Stream.Reader(msg.reader, msg.buffer)
		for (;;) {
			const data = await stream.read()
			if (!data) break

			this.init.push(data)
		}

		// TODO make sure the init segment is fully received
	}

	async receiveSegment(msg: Message.Segment) {
		// Wait for the init segment to be fully received and parsed
		const init = await this.init.info
		const input = MP4.New()

		input.onSamples = this.onSamples.bind(this)
		input.onReady = (track: any) => {
			// Extract all of the tracks, because we don't know if it's audio or video.
			for (const i of init.tracks) {
				input.setExtractionOptions(track.id, i, { nbSamples: 1 })
			}

			input.start()
		}

		// MP4box requires us to reparse the init segment unfortunately
		let offset = 0

		for (const raw of this.init.raw) {
			raw.fileStart = offset
			offset = input.appendBuffer(raw)
		}

		const stream = new Stream.Reader(msg.reader, msg.buffer)

		// For whatever reason, mp4box doesn't work until you read an atom at a time.
		while (!(await stream.done())) {
			const raw = await stream.peek(4)

			// TODO this doesn't support when size = 0 (until EOF) or size = 1 (extended size)
			const size = new DataView(
				raw.buffer,
				raw.byteOffset,
				raw.byteLength
			).getUint32(0)
			const atom = await stream.bytes(size)

			// Make a copy of the atom because mp4box only accepts an ArrayBuffer unfortunately
			const box = new Uint8Array(atom.byteLength)
			box.set(atom)

			// and for some reason we need to modify the underlying ArrayBuffer with offset
			const buffer = box.buffer as MP4.ArrayBuffer
			buffer.fileStart = offset

			// Parse the data
			offset = input.appendBuffer(buffer)
			input.flush()
		}
	}

	onSamples(track_id: number, track: MP4.Track, samples: MP4.Sample[]) {
		let decoder = this.decoders.get(track_id)

		if (!decoder) {
			// We need a sample to initalize the video decoder, because of mp4box limitations.
			const sample = samples[0]

			if (isVideoTrack(track)) {
				// Configure the decoder using the AVC box for H.264
				// TODO it should be easy to support other codecs, just need to know the right boxes.
				const avcc = sample.description.avcC
				if (!avcc) throw new Error("TODO only h264 is supported")

				const description = new MP4.Stream(
					new Uint8Array(avcc.size),
					0,
					false
				)
				avcc.write(description)

				const videoDecoder = new VideoDecoder({
					output: this.renderer.push.bind(this.renderer),
					error: console.warn,
				})

				videoDecoder.configure({
					codec: track.codec,
					codedHeight: track.video.height,
					codedWidth: track.video.width,
					description: description.buffer?.slice(8),
					// optimizeForLatency: true
				})

				decoder = videoDecoder
			} else if (isAudioTrack(track)) {
				const audioDecoder = new AudioDecoder({
					output: this.renderer.push.bind(this.renderer),
					error: console.warn,
				})

				audioDecoder.configure({
					codec: track.codec,
					numberOfChannels: track.audio.channel_count,
					sampleRate: track.audio.sample_rate,
				})

				decoder = audioDecoder
			} else {
				throw new Error("unknown track type")
			}

			this.decoders.set(track_id, decoder)
		}

		for (const sample of samples) {
			// Convert to microseconds
			const timestamp = (1000 * 1000 * sample.dts) / sample.timescale
			const duration = (1000 * 1000 * sample.duration) / sample.timescale

			if (isAudioDecoder(decoder)) {
				decoder.decode(
					new EncodedAudioChunk({
						type: sample.is_sync ? "key" : "delta",
						data: sample.data,
						duration: duration,
						timestamp: timestamp,
					})
				)
			} else if (isVideoDecoder(decoder)) {
				decoder.decode(
					new EncodedVideoChunk({
						type: sample.is_sync ? "key" : "delta",
						data: sample.data,
						duration: duration,
						timestamp: timestamp,
					})
				)
			} else {
				throw new Error("unknown decoder type")
			}
		}
	}
}

function isAudioDecoder(
	decoder: AudioDecoder | VideoDecoder
): decoder is AudioDecoder {
	return decoder instanceof AudioDecoder
}

function isVideoDecoder(
	decoder: AudioDecoder | VideoDecoder
): decoder is VideoDecoder {
	return decoder instanceof VideoDecoder
}

function isAudioTrack(track: MP4.Track): track is MP4.AudioTrack {
	return (track as MP4.AudioTrack).audio !== undefined
}

function isVideoTrack(track: MP4.Track): track is MP4.VideoTrack {
	return (track as MP4.VideoTrack).video !== undefined
}
