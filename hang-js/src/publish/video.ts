import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import { Frame } from "../container/frame";
import { VideoTrackSettings } from "../util/settings";
import { Deferred } from "../util/async";
import * as Hex from "../util/hex";

// Create a group every 2 seconds
const GOP_DURATION_US = 2 * 1000 * 1000;

export class Video {
	readonly id: string;
	readonly track: Moq.Track;
	readonly encoderConfig: VideoEncoderConfig;
	readonly decoderConfig = new Deferred<VideoDecoderConfig>();

	#encoder: VideoEncoder;
	#reader: ReadableStreamDefaultReader<VideoFrame>;

	// The current group and the timestamp of the first frame in it.
	#group?: Moq.GroupWriter;
	#groupTimestamp = 0;

	constructor(id: string, reader: ReadableStreamDefaultReader<VideoFrame>, config: VideoEncoderConfig) {
		this.id = id;
		this.#reader = reader;
		this.track = new Moq.Track("video", 2);
		this.encoderConfig = config;
		this.#encoder = new VideoEncoder({
			output: (frame, metadata) => this.#encoded(frame, metadata),
			error: (err) => this.track.writer.abort(err),
		});

		this.#encoder.configure(config);

		this.#run()
			.catch((err) => {
				this.track.writer.abort(err);
				this.decoderConfig.reject(err);
			})
			.finally(() => {
				this.#reader.cancel();
				this.track.writer.close();
			});
	}

	// This is called instead of the constructor to determine the best configuration.
	static async create(media: MediaStreamVideoTrack): Promise<Video> {
		// `getSettings` is not reliable, so we read the first frame to determine the resolution.
		const processor = new MediaStreamTrackProcessor({ track: media });
		const reader = processor.readable.getReader();
		const { value: frame } = await reader.read();
		if (!frame) throw new Error("no first frame");

		// But we still use the settings to determine the framerate.
		const settings = media.getSettings() as VideoTrackSettings;

		const config = await Video.config(settings, frame);
		frame.close();

		return new Video(media.id, reader, config);
	}

	// Try to determine the best config for the given settings.
	static async config(settings: VideoTrackSettings, frame: VideoFrame): Promise<VideoEncoderConfig> {
		const width = frame.codedWidth;
		const height = frame.codedHeight;

		// TARGET BITRATE CALCULATION
		// 480p@30 = 1.0mbps
		// 480p@60 = 1.5mbps
		// 720p@30 = 2.5mbps
		// 720p@60 = 3.5mpbs
		// 1080p@30 = 4.5mbps
		// 1080p@60 = 6.0mbps
		const pixels = width * height;

		// 30fps is the baseline, applying a multiplier for higher framerates.
		// Framerate does not cause a multiplicative increase in bitrate because of delta encoding.
		// TODO Make this better.
		const framerateFactor = 30.0 + (settings.frameRate - 30) / 2;
		const bitrate = Math.round(pixels * 0.07 * framerateFactor);

		// ACTUAL BITRATE CALCULATION (h264)
		// 480p@30 = 409920 * 30 * 0.07 = 0.9 Mb/s
		// 480p@60 = 409920 * 45 * 0.07 = 1.3 Mb/s
		// 720p@30 = 921600 * 30 * 0.07 = 1.9 Mb/s
		// 720p@60 = 921600 * 45 * 0.07 = 2.9 Mb/s
		// 1080p@30 = 2073600 * 30 * 0.07 = 4.4 Mb/s
		// 1080p@60 = 2073600 * 45 * 0.07 = 6.5 Mb/s

		const baseConfig: VideoEncoderConfig = {
			codec: "none",
			width,
			height,
			bitrate,
			latencyMode: "realtime",
			framerate: settings.frameRate,
		};

		// A list of codecs to try, in order of preference.
		const CODECS: { codec: string; scale: number; hardware: boolean }[] = [
			// We try hardware encoding first
			// AV1
			{ codec: "av01.0.08M.08", scale: 0.6, hardware: true },
			{ codec: "av01", scale: 0.6, hardware: true }, // Browser's choice

			// HEVC (aka h.265)
			{ codec: "hev1.1.6.L93.B0", scale: 0.8, hardware: true },
			{ codec: "hev1", scale: 0.8, hardware: true }, // Browser's choice

			// VP9
			{ codec: "vp09.00.10.08", scale: 0.8, hardware: true },
			{ codec: "vp9", scale: 0.8, hardware: true }, // Browser's choice

			// H.264
			{ codec: "avc1.640028", scale: 1.0, hardware: true }, // High
			{ codec: "avc1.4D401F", scale: 1.0, hardware: true }, // Main
			{ codec: "avc1.42E01E", scale: 1.0, hardware: true }, // Baseline
			{ codec: "avc1", scale: 1.0, hardware: true }, // Browser's choice

			// VP8
			{ codec: "vp8", scale: 1.1, hardware: true },

			// Now try software encoding for simple enough codecs.
			// H.264
			{ codec: "avc1.640028", scale: 1.0, hardware: false }, // High
			{ codec: "avc1.4D401F", scale: 1.0, hardware: false }, // Main
			{ codec: "avc1.42E01E", scale: 1.0, hardware: false }, // Baseline
			{ codec: "avc1", scale: 1.0, hardware: false },

			// VP8
			{ codec: "vp8", scale: 1.1, hardware: false },

			// VP9
			// It's a bit more expensive to encode so we shy away from it.
			{ codec: "vp09.00.10.08", scale: 0.8, hardware: false },
			{ codec: "vp9", scale: 0.8, hardware: false },

			// HEVC (aka h.265)
			// This likely won't work because of licensing issues.
			{ codec: "hev1.1.6.L93.B0", scale: 0.8, hardware: false },
			{ codec: "hev1", scale: 0.8, hardware: false }, // Browser's choice

			// AV1
			// Super expensive to encode so it's our last choice.
			{ codec: "av01.0.08M.08", scale: 0.6, hardware: false },
			{ codec: "av01", scale: 0.6, hardware: false },
		];

		// Try hardware encoding first.
		for (const codec of CODECS) {
			const config: VideoEncoderConfig = {
				...baseConfig,
				codec: codec.codec,
				bitrate: Math.round(bitrate * codec.scale),
				hardwareAcceleration: codec.hardware ? "prefer-hardware" : "prefer-software",
			};

			const { supported } = await VideoEncoder.isConfigSupported(config);
			if (supported) return config;
		}

		throw new Error("no supported codec");
	}

	// Returns the catalog for the configured settings.
	async catalog(): Promise<Catalog.Video> {
		const encoderConfig = this.encoderConfig;
		const decoderConfig = await this.decoderConfig.promise;

		const description = decoderConfig.description ? Hex.encode(decoderConfig.description) : undefined;

		return {
			track: {
				name: this.track.name,
				priority: this.track.priority,
			},
			codec: decoderConfig.codec,
			description,
			resolution: {
				width: encoderConfig.width,
				height: encoderConfig.height,
			},
			framerate: encoderConfig.framerate,
			bitrate: encoderConfig.bitrate,
		};
	}

	async #run() {
		for (;;) {
			const result = await this.#reader.read();
			if (!result || result.done) {
				break;
			}

			const frame = result.value;

			// Create a keyframe at least every 2 seconds.
			// We set this to undefined not `false` so the encoder can decide too.
			const keyFrame = this.#groupTimestamp + GOP_DURATION_US < frame.timestamp || undefined;

			this.#encoder.encode(frame, { keyFrame });
			frame.close();
		}
	}

	async #encoded(frame: EncodedVideoChunk, metadata?: EncodedVideoChunkMetadata) {
		if (metadata?.decoderConfig) {
			this.decoderConfig.resolve(metadata.decoderConfig);
		}

		if (frame.type === "key") {
			this.#groupTimestamp = frame.timestamp;
			this.#group?.close();
			this.#group = await this.track.writer.appendGroup();
		}

		if (!this.#group) throw new Error("missing keyframe");

		const buffer = new Uint8Array(frame.byteLength);
		frame.copyTo(buffer);

		const karp = new Frame(frame.type === "key", frame.timestamp, buffer);
		karp.encode(this.#group);
	}

	close() {
		this.track.writer.close();
		this.#encoder.close();
	}
}
