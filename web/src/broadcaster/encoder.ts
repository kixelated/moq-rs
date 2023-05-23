import * as MP4 from "../mp4"

export class Encoder {
	container: MP4.ISOFile
	audio: AudioEncoder
	video: VideoEncoder

	constructor() {
		this.container = new MP4.ISOFile()

		this.audio = new AudioEncoder({
			output: this.onAudio.bind(this),
			error: console.warn,
		})

		this.video = new VideoEncoder({
			output: this.onVideo.bind(this),
			error: console.warn,
		})

		this.container.init()

		this.audio.configure({
			codec: "mp4a.40.2",
			numberOfChannels: 2,
			sampleRate: 44100,

			// TODO bitrate
		})

		this.video.configure({
			codec: "avc1.42002A", // TODO h.264 baseline
			avc: { format: "avc" }, // or annexb
			width: 1280,
			height: 720,

			// TODO bitrate
			// TODO bitrateMode
			// TODO framerate
			// TODO latencyMode
		})
	}

	onAudio(frame: EncodedAudioChunk, metadata: EncodedAudioChunkMetadata) {
		const config = metadata.decoderConfig!
		const track_id = 1

		if (!this.container.getTrackById(track_id)) {
			this.container.addTrack({
				id: track_id,
				type: "mp4a", // TODO wrong
				timescale: 1000, // TODO verify

				channel_count: config.numberOfChannels,
				samplerate: config.sampleRate,

				description: config.description, // TODO verify
				// TODO description_boxes?: Box[];
			})
		}

		const buffer = new Uint8Array(frame.byteLength)
		frame.copyTo(buffer)

		// TODO cts?
		const sample = this.container.addSample(track_id, buffer, {
			is_sync: frame.type == "key",
			duration: frame.duration!,
			dts: frame.timestamp,
		})

		const _stream = this.container.createSingleSampleMoof(sample)
	}

	onVideo(frame: EncodedVideoChunk, metadata?: EncodedVideoChunkMetadata) {
		const config = metadata!.decoderConfig!
		const track_id = 2

		if (!this.container.getTrackById(track_id)) {
			this.container.addTrack({
				id: 2,
				type: "avc1",
				width: config.codedWidth,
				height: config.codedHeight,
				timescale: 1000, // TODO verify

				description: config.description, // TODO verify
				// TODO description_boxes?: Box[];
			})
		}

		const buffer = new Uint8Array(frame.byteLength)
		frame.copyTo(buffer)

		// TODO cts?
		const sample = this.container.addSample(track_id, buffer, {
			is_sync: frame.type == "key",
			duration: frame.duration!,
			dts: frame.timestamp,
		})

		const _stream = this.container.createSingleSampleMoof(sample)
	}
}
