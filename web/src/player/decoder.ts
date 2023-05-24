import * as Message from "./message"
import * as MP4 from "../mp4"
import * as Stream from "../stream"

import Renderer from "./renderer"
import { Deferred } from "../util"

export default class Decoder {
	decoders: Map<number, AudioDecoder | VideoDecoder>
	renderer: Renderer

	init: Deferred<MP4.ArrayBuffer[]>

	constructor(renderer: Renderer) {
		this.init = new Deferred()
		this.decoders = new Map()
		this.renderer = renderer
	}

	async receiveInit(msg: Message.Init) {
		const init = new Array<MP4.ArrayBuffer>()
		let offset = 0

		const stream = new Stream.Reader(msg.reader, msg.buffer)
		for (;;) {
			const data = await stream.read()
			if (!data) break

			// Make a copy of the atom because mp4box only accepts an ArrayBuffer unfortunately
			const box = new Uint8Array(data.byteLength)
			box.set(data)

			// and for some reason we need to modify the underlying ArrayBuffer with fileStart
			const buffer = box.buffer as MP4.ArrayBuffer
			buffer.fileStart = offset

			// Add the box to our queue of chunks
			init.push(buffer)

			offset += data.byteLength
		}

		this.init.resolve(init)
	}

	async receiveSegment(msg: Message.Segment) {
		// Wait for the init segment to be fully received and parsed
		const input = MP4.New()

		input.onSamples = this.onSamples.bind(this)
		input.onReady = (info: MP4.Info) => {
			// Extract all of the tracks, because we don't know if it's audio or video.
			for (const track of info.tracks) {
				input.setExtractionOptions(track.id, track, { nbSamples: 1 })
			}

			input.start()
		}

		// MP4box requires us to parse the init segment for each segment unfortunately
		// TODO If this sees production usage, I would recommend caching this somehow.
		let offset = 0

		const init = await this.init.promise
		for (const raw of init) {
			offset = input.appendBuffer(raw)
		}

		const stream = new Stream.Reader(msg.reader, msg.buffer)

		// For whatever reason, mp4box doesn't work until you read an atom at a time.
		while (!(await stream.done())) {
			const raw = await stream.peek(4)

			// TODO this doesn't support when size = 0 (until EOF) or size = 1 (extended size)
			const size = new DataView(raw.buffer, raw.byteOffset, raw.byteLength).getUint32(0)
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

	onSamples(_track_id: number, track: MP4.Track, samples: MP4.Sample[]) {
		if (!track.track_width) {
			// TODO ignoring audio to debug
			return
		}

		let decoder
		if (isVideoTrack(track)) {
			// We need a sample to initalize the video decoder, because of mp4box limitations.
			decoder = this.videoDecoder(track, samples[0])
		} else if (isAudioTrack(track)) {
			decoder = this.audioDecoder(track)
		} else {
			throw new Error("unknown track type")
		}

		for (const sample of samples) {
			// Convert to microseconds
			const timestamp = (1000 * 1000 * sample.dts) / sample.timescale
			const duration = (1000 * 1000 * sample.duration) / sample.timescale

			if (!decoder) {
				throw new Error("decoder not initialized")
			} else if (isAudioDecoder(decoder)) {
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

	audioDecoder(track: MP4.AudioTrack): AudioDecoder {
		// Reuse the audio decoder when possible to avoid glitches.
		// TODO detect when the codec changes and make a new decoder.
		const decoder = this.decoders.get(track.id)
		if (decoder && isAudioDecoder(decoder)) {
			return decoder
		}

		const audioDecoder = new AudioDecoder({
			output: this.renderer.push.bind(this.renderer),
			error: console.error,
		})

		audioDecoder.configure({
			codec: track.codec,
			numberOfChannels: track.audio.channel_count,
			sampleRate: track.audio.sample_rate,
		})

		this.decoders.set(track.id, audioDecoder)

		return audioDecoder
	}

	videoDecoder(track: MP4.VideoTrack, sample: MP4.Sample): VideoDecoder {
		// Make a new video decoder for each keyframe.
		if (!sample.is_sync) {
			const decoder = this.decoders.get(track.id)
			if (decoder && isVideoDecoder(decoder)) {
				return decoder
			}
		}

		// Configure the decoder using the AVC box for H.264
		// TODO it should be easy to support other codecs, just need to know the right boxes.
		const avcc = sample.description.avcC
		if (!avcc) throw new Error("TODO only h264 is supported")

		const description = new MP4.Stream(new Uint8Array(avcc.size), 0, false)
		avcc.write(description)

		const videoDecoder = new VideoDecoder({
			output: this.renderer.push.bind(this.renderer),
			error: console.error,
		})

		videoDecoder.configure({
			codec: track.codec,
			codedHeight: track.video.height,
			codedWidth: track.video.width,
			description: description.buffer?.slice(8),
			// optimizeForLatency: true
		})

		this.decoders.set(track.id, videoDecoder)

		return videoDecoder
	}
}

function isAudioDecoder(decoder: AudioDecoder | VideoDecoder): decoder is AudioDecoder {
	return decoder instanceof AudioDecoder
}

function isVideoDecoder(decoder: AudioDecoder | VideoDecoder): decoder is VideoDecoder {
	return decoder instanceof VideoDecoder
}

function isAudioTrack(track: MP4.Track): track is MP4.AudioTrack {
	return (track as MP4.AudioTrack).audio !== undefined
}

function isVideoTrack(track: MP4.Track): track is MP4.VideoTrack {
	return (track as MP4.VideoTrack).video !== undefined
}
