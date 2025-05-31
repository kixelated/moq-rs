import { Buffer } from "buffer";
import * as Moq from "@kixelated/moq";
import { Signal, Signals, cleanup, signal } from "@kixelated/signals";
import * as Catalog from "../catalog";
import * as Container from "../container";

// Create a group every 2 seconds
const GOP_DURATION_US = 2 * 1000 * 1000;

// Stronger typing for the MediaStreamTrack interface.
export interface VideoTrack extends MediaStreamTrack {
	kind: "video";
	clone(): VideoTrack;
}

export interface VideoTrackSettings {
	deviceId: string;
	groupId: string;

	aspectRatio: number;
	facingMode: "user" | "environment" | "left" | "right";
	frameRate: number;
	height: number;
	resizeMode: "none" | "crop-and-scale";
	width: number;
}

export type VideoConstraints = Omit<
	MediaTrackConstraints,
	"autoGainControl" | "channelCount" | "echoCancellation" | "noiseSuppression" | "sampleRate" | "sampleSize"
> & {
	// TODO update @types/web
	resizeMode?: "none" | "crop-and-scale";
};

export type VideoProps = {
	media?: VideoTrack;
	constraints?: VideoConstraints | boolean;
};

export class Video {
	broadcast: Moq.BroadcastProducer;

	readonly media: Signal<VideoTrack | undefined>;
	readonly constraints: Signal<VideoConstraints | boolean | undefined>;

	#catalog = signal<Catalog.Video | undefined>(undefined);
	readonly catalog = this.#catalog.readonly();

	#track = signal<Moq.TrackProducer | undefined>(undefined);

	#active = signal(false);
	readonly active = this.#active.readonly();

	#encoderConfig = signal<VideoEncoderConfig | undefined>(undefined);
	#decoderConfig = signal<VideoDecoderConfig | undefined>(undefined);

	#group?: Moq.GroupProducer;
	#groupTimestamp = 0;

	#signals = new Signals();
	#id = 0;

	// Store the latest VideoFrame and when it was captured.
	#latest: { frame: VideoFrame; when: DOMHighResTimeStamp } | undefined;

	constructor(broadcast: Moq.BroadcastProducer, props?: VideoProps) {
		this.broadcast = broadcast;
		this.media = signal(props?.media);
		this.constraints = signal(props?.constraints);

		this.#signals.effect(() => this.#runTrack());
		this.#signals.effect(() => this.#runEncoder());
		this.#signals.effect(() => this.#runCatalog());
	}

	#runTrack() {
		const media = this.media.get();
		if (!media) return;

		const track = new Moq.TrackProducer(`video-${this.#id++}`, 1);
		cleanup(() => track.close());

		this.broadcast.insertTrack(track.consume());
		cleanup(() => this.broadcast.removeTrack(track.name));

		this.#track.set(track);
		cleanup(() => this.#track.set(undefined));
	}

	#runEncoder() {
		const media = this.media.get();
		if (!media) return;

		const track = this.#track.get();
		if (!track) return;

		const settings = media.getSettings() as VideoTrackSettings;
		const processor = VideoTrackProcessor(media);
		const reader = processor.getReader();

		const encoder = new VideoEncoder({
			output: (frame: EncodedVideoChunk, metadata?: EncodedVideoChunkMetadata) => {
				if (metadata?.decoderConfig) {
					this.#decoderConfig.set(metadata.decoderConfig);
				}

				if (frame.type === "key") {
					this.#groupTimestamp = frame.timestamp;
					this.#group?.close();
					this.#group = track.appendGroup();
				} else if (!this.#group) {
					throw new Error("no keyframe");
				}

				const buffer = new Uint8Array(frame.byteLength);
				frame.copyTo(buffer);

				const container = new Container.Frame(frame.type === "key", frame.timestamp, buffer);
				container.encode(this.#group);
			},
			error: (err: Error) => {
				this.#group?.abort(err);
				this.#group = undefined;

				track.abort(err);
			},
		});

		this.#encoderConfig.set(undefined);
		this.#decoderConfig.set(undefined);

		void this.#runFrames(encoder, reader, settings);

		return () => {
			reader.cancel();
		};
	}

	async #runFrames(
		encoder: VideoEncoder,
		reader: ReadableStreamDefaultReader<VideoFrame>,
		settings: VideoTrackSettings,
	) {
		try {
			let { value: frame } = await reader.read();
			if (!frame) return;

			this.#active.set(true);

			const config = await Video.#bestEncoderConfig(settings, frame);
			encoder.configure(config);

			this.#encoderConfig.set(config);

			while (frame) {
				const keyFrame = this.#groupTimestamp + GOP_DURATION_US < frame.timestamp;
				if (keyFrame) {
					this.#groupTimestamp = frame.timestamp;
				}

				this.#latest?.frame.close();
				this.#latest = { frame, when: performance.now() };

				encoder.encode(frame, { keyFrame });

				({ value: frame } = await reader.read());
			}
		} finally {
			encoder.close();
			this.#active.set(false);

			this.#group?.close();
			this.#group = undefined;
		}
	}

	// Try to determine the best config for the given settings.
	static async #bestEncoderConfig(settings: VideoTrackSettings, frame: VideoFrame): Promise<VideoEncoderConfig> {
		const width = frame.codedWidth;
		const height = frame.codedHeight;
		const framerate = settings.frameRate;

		console.debug("determining best encoder config for: ", {
			width,
			height,
			framerate,
		});

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
		const framerateFactor = 30.0 + (framerate - 30) / 2;
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
			// VP9
			// More likely to have hardware decoding, but hardware encoding is less likely.
			"vp09.00.10.08",
			"vp09", // Browser's choice

			// H.264
			// Almost always has hardware encoding and decoding.
			"avc1.640028",
			"avc1.4D401F",
			"avc1.42E01E",
			"avc1",

			// AV1
			// One day will get moved higher up the list, but hardware decoding is rare.
			"av01.0.08M.08",
			"av01",

			// HEVC (aka h.265)
			// More likely to have hardware encoding, but less likely to be supported (licensing issues).
			// Unfortunately, Firefox doesn't support decoding so it's down here at the bottom.
			"hev1.1.6.L93.B0",
			"hev1", // Browser's choice

			// VP8
			// A terrible codec but it's easy.
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
			framerate,
		};

		// Try hardware encoding first.
		// We can't reliably detect hardware encoding on Firefox: https://github.com/w3c/webcodecs/issues/896
		if (!navigator.userAgent.toLowerCase().includes("firefox")) {
			for (const codec of HARDWARE_CODECS) {
				const config = Video.#codecSpecific(baseConfig, codec, bitrate, true);
				const { supported, config: hardwareConfig } = await VideoEncoder.isConfigSupported(config);
				if (supported && hardwareConfig) {
					console.debug("using hardware encoding: ", hardwareConfig);
					return hardwareConfig;
				}
			}
		} else {
			console.warn("Cannot detect hardware encoding on Firefox.");
		}

		// Try software encoding.
		for (const codec of SOFTWARE_CODECS) {
			const config = Video.#codecSpecific(baseConfig, codec, bitrate, false);
			const { supported, config: softwareConfig } = await VideoEncoder.isConfigSupported(config);
			if (supported && softwareConfig) {
				console.debug("using software encoding: ", softwareConfig);
				return softwareConfig;
			}
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
			hardwareAcceleration: hardware ? "prefer-hardware" : undefined,
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

		const track = this.#track.get();
		if (!track) return;

		const description = decoderConfig.description
			? Buffer.from(decoderConfig.description as Uint8Array).toString("hex")
			: undefined;

		const catalog: Catalog.Video = {
			track: {
				name: track.name,
				priority: track.priority,
			},
			config: {
				// The order is important here.
				...encoderConfig,
				...decoderConfig,
				description,
			},
		};

		this.#catalog.set(catalog);

		return () => {
			this.#catalog.set(undefined);
		};
	}

	frame(now: DOMHighResTimeStamp): { frame: VideoFrame; lag: DOMHighResTimeStamp } | undefined {
		if (!this.#latest) return;

		const lag = now - this.#latest.when;
		return { frame: this.#latest.frame, lag };
	}

	close() {
		this.#latest?.frame.close();
		this.#signals.close();
	}
}

// Firefox doesn't support MediaStreamTrackProcessor so we need to use a polyfill.
// Based on: https://jan-ivar.github.io/polyfills/mediastreamtrackprocessor.js
// Thanks Jan-Ivar
function VideoTrackProcessor(track: VideoTrack): ReadableStream<VideoFrame> {
	// @ts-expect-error Chrome only for now
	if (self.MediaStreamTrackProcessor) {
		// @ts-expect-error Chrome only for now
		return new self.MediaStreamTrackProcessor({ track }).readable;
	}

	console.warn("Using MediaStreamTrackProcessor polyfill; performance might suffer.");

	const settings = track.getSettings();
	if (!settings) {
		throw new Error("track has no settings");
	}

	let video: HTMLVideoElement;
	let canvas: HTMLCanvasElement;
	let ctx: CanvasRenderingContext2D;
	let last: DOMHighResTimeStamp;

	const frameRate = settings.frameRate ?? 30;

	return new ReadableStream<VideoFrame>({
		async start() {
			video = document.createElement("video") as HTMLVideoElement;
			video.srcObject = new MediaStream([track]);
			await Promise.all([
				video.play(),
				new Promise((r) => {
					video.onloadedmetadata = r;
				}),
			]);
			// TODO use offscreen canvas
			canvas = document.createElement("canvas");
			canvas.width = video.videoWidth;
			canvas.height = video.videoHeight;

			const c = canvas.getContext("2d", { desynchronized: true });
			if (!c) {
				throw new Error("failed to create canvas context");
			}
			ctx = c;
			last = performance.now();
		},
		async pull(controller) {
			while (true) {
				const now = performance.now();
				if (now - last < 1000 / frameRate) {
					await new Promise((r) => requestAnimationFrame(r));
					continue;
				}

				last = now;
				ctx.drawImage(video, 0, 0);
				controller.enqueue(new VideoFrame(canvas, { timestamp: last * 1000 }));
			}
		},
	});
}
