import * as Moq from "@kixelated/moq";
import * as Media from "../media";

import { VideoTrackSettings } from "../util/settings";

import { Buffer } from "buffer";
import { Signal, Signals, signal } from "../signals"

// Create a group every 2 seconds
const GOP_DURATION_US = 2 * 1000 * 1000;

export type VideoTrackConstraints = Omit<MediaTrackConstraints,
  | "autoGainControl"
  | "channelCount"
  | "echoCancellation"
  | "noiseSuppression"
  | "sampleRate"
  | "sampleSize"
> & {
	// TODO update @types/web
	resizeMode?: "none" | "crop-and-scale";
};

export type VideoProps = {
	media?: MediaStreamVideoTrack;
	constraints?: VideoTrackConstraints | boolean;
};

export class Video {
	readonly media: Signal<MediaStreamVideoTrack | undefined>;
	readonly constraints: Signal<VideoTrackConstraints | boolean | undefined>;

	#catalog = signal<Media.Video | undefined>(undefined);
	readonly catalog = this.#catalog.readonly();

	#track = signal<Moq.Track | undefined>(undefined);
	readonly track = this.#track.readonly();

	#encoderConfig = signal<VideoEncoderConfig | undefined>(undefined);
	#decoderConfig = signal<VideoDecoderConfig | undefined>(undefined);

	#group?: Moq.GroupWriter;
	#groupTimestamp = 0;

	#signals = new Signals();
	#id = 0;

	constructor(props?: VideoProps) {
		this.media = signal(props?.media);
		this.constraints = signal(props?.constraints);

		this.#signals.effect(() => this.#runTrack());
		this.#signals.effect(() => this.#runEncoder());
		this.#signals.effect(() => this.#runCatalog());
	}

	#runTrack() {
		const media = this.media.get();
		if (!media) return;

		const track = new Moq.Track(`video-${this.#id++}`, 1);
		this.#track.set(track);

		return () => {
			track.close();
			this.#track.set(undefined);
		}
	}

	#runEncoder() {
		const media = this.media.get();
		if (!media) return;

		const track = this.track.get();
		if (!track) return;

		const settings = media.getSettings() as VideoTrackSettings;
		console.log("settings:", settings);

		const processor = new MediaStreamTrackProcessor({ track: media });
		const reader = processor.readable.getReader();

		const encoder = new VideoEncoder({
			output: (frame: EncodedVideoChunk, metadata?: EncodedVideoChunkMetadata) => {
				if (metadata?.decoderConfig) {
					this.#decoderConfig.set(metadata.decoderConfig);
				}

				if (frame.type === "key") {
					this.#groupTimestamp = frame.timestamp;
					this.#group?.close();
					this.#group = track.writer.appendGroup();
				} else if (!this.#group) {
					throw new Error("no keyframe");
				}

				const buffer = new Uint8Array(frame.byteLength);
				frame.copyTo(buffer);

				const container = new Media.Frame(frame.type === "key", frame.timestamp, buffer);
				container.encode(this.#group);
			},
			error: (err: Error) => {
				this.#group?.abort(err);
				this.#group = undefined;

				track.writer.abort(err);
			}
		});

		this.#encoderConfig.set(undefined);
		this.#decoderConfig.set(undefined);

		(async () => {
			let { value: frame } = await reader.read();
			if (!frame) return;

			const config = await Video.#bestEncoderConfig(settings, frame);
			encoder.configure(config);

			this.#encoderConfig.set(config);

			while (frame) {
				const keyFrame = this.#groupTimestamp + GOP_DURATION_US < frame.timestamp || undefined;
				this.#groupTimestamp = frame.timestamp;

				encoder.encode(frame, { keyFrame });
				frame.close();

				({ value: frame } = await reader.read());
			}

			try {
				encoder.close();

				this.#group?.close();
				this.#group = undefined;
			} catch {}
		})();

		return () => {
			reader.cancel();
		}
	}

	// Try to determine the best config for the given settings.
	static async #bestEncoderConfig(settings: VideoTrackSettings, frame: VideoFrame): Promise<VideoEncoderConfig> {
		const width = frame.codedWidth;
		const height = frame.codedHeight;

		// TARGET BITRATE CALCULATION (h264)
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

		// ACTUAL BITRATE CALCULATION
		// 480p@30 = 409920 * 30 * 0.07 = 0.9 Mb/s
		// 480p@60 = 409920 * 45 * 0.07 = 1.3 Mb/s
		// 720p@30 = 921600 * 30 * 0.07 = 1.9 Mb/s
		// 720p@60 = 921600 * 45 * 0.07 = 2.9 Mb/s
		// 1080p@30 = 2073600 * 30 * 0.07 = 4.4 Mb/s
		// 1080p@60 = 2073600 * 45 * 0.07 = 6.5 Mb/s

		// A list of codecs to try, in order of preference.
		const HARDWARE_CODECS = [
			// AV1
			"av01.0.08M.08",
			"av01",

			// HEVC (aka h.265)
			"hev1.1.6.L93.B0",
			"hev1", // Browser's choice

			// VP9
			"vp09.00.10.08",
			"vp09", // Browser's choice

			// H.264
			"avc1.640028",
			"avc1.4D401F",
			"avc1.42E01E",
			"avc1",

			// VP8
			"vp8",
		];

		const SOFTWARE_CODECS = [
			// Now try software encoding for simple enough codecs.
			// H.264
			"avc1.640028", // High
			"avc1.4D401F", // Main
			"avc1.42E01E", // Baseline
			"avc1",

			// VP8
			"vp8",

			// VP9
			// It's a bit more expensive to encode so we shy away from it.
			"vp09.00.10.08",
			"vp09",

			// HEVC (aka h.265)
			// This likely won't work because of licensing issues.
			"hev1.1.6.L93.B0",
			"hev1", // Browser's choice

			// AV1
			// Super expensive to encode so it's our last choice.
			"av01.0.08M.08",
			"av01",
		];

		const baseConfig: VideoEncoderConfig = {
			codec: "none",
			width,
			height,
			bitrate,
			latencyMode: "realtime",
			framerate: settings.frameRate,
		};

		// Try hardware encoding first.
		for (const codec of HARDWARE_CODECS) {
			const config = Video.#codecSpecific(baseConfig, codec, bitrate, true);
			const { supported } = await VideoEncoder.isConfigSupported(config);
			if (supported) return config;
		}

		// Try software encoding.
		for (const codec of SOFTWARE_CODECS) {
			const config = Video.#codecSpecific(baseConfig, codec, bitrate, false);
			const { supported } = await VideoEncoder.isConfigSupported(config);
			if (supported) return config;
		}

		throw new Error("no supported codec");
	}

	// Modify the config for codec specific settings.
	static #codecSpecific(
		base: VideoEncoderConfig,
		codec: string,
		bitrate: number,
		hardware: boolean,
	): VideoEncoderConfig {
		const config: VideoEncoderConfig = {
			...base,
			codec,
			hardwareAcceleration: hardware ? "prefer-hardware" : "prefer-software",
		};

		// We scale the bitrate for more efficient codecs.
		// TODO This shouldn't be linear, as the efficiency is very similar at low bitrates.
		if (config.codec.startsWith("avc1")) {
			// Annex-B allows changing the resolution without nessisarily updating the catalog (description).
			config.avc = { format: "annexb" };
		} else if (config.codec.startsWith("hev1")) {
			// Annex-B allows changing the resolution without nessisarily updating the catalog (description).
			// @ts-ignore Typescript needs to be updated.
			config.hevc = { format: "annexb" };
		} else if (config.codec.startsWith("vp09")) {
			config.bitrate = bitrate * 0.8;
		} else if (config.codec.startsWith("av01")) {
			config.bitrate = bitrate * 0.6;
		} else if (config.codec === "vp8") {
			// Worse than H.264 but it's a backup plan.
			config.bitrate = bitrate * 1.1;
		}

		return config;
	}

	// Returns the catalog for the configured settings.
	#runCatalog() {
		const encoderConfig = this.#encoderConfig.get();
		if (!encoderConfig) return;

		const decoderConfig = this.#decoderConfig.get();
		if (!decoderConfig) return;

		const track = this.track.get();
		if (!track) return;

		const description = decoderConfig.description
			? Buffer.from(decoderConfig.description as Uint8Array).toString("hex")
			: undefined;

		const catalog: Media.Video = {
			track: {
				name: track.name,
				priority: track.priority,
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

		this.#catalog.set(catalog);

		return () => {
			this.#catalog.set(undefined);
		}
	}

	close() {
		this.#signals.close();
	}
}
