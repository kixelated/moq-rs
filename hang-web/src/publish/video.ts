import * as Moq from "@kixelated/moq";
import { Frame } from "../catalog/frame";
import { Deferred } from "../util/async";
import { Context } from "../util/context";

const SUPPORTED = [
	"avc1", // H.264
	"hev1", // HEVC (aka h.265)
	// "av01", // TDOO support AV1
];

export interface EncoderSupported {
	codecs: string[];
}

export type EncoderConfig = VideoEncoderConfig;

export class Encoder {
	#outputTrack: Moq.Track;
	#outputGroup?: Moq.Group;

	#encoder: VideoEncoder;
	#encoderConfig: VideoEncoderConfig;

	#decoder = new Deferred<VideoDecoderConfig>();

	// true if we should insert a keyframe, undefined when the encoder should decide
	#keyframeNext: true | undefined = true;

	// Count the number of frames without a keyframe.
	#keyframeCounter = 0;

	constructor(config: VideoEncoderConfig, output: Moq.Track) {
		this.#outputTrack = output;

		config.bitrateMode ??= "constant";
		config.latencyMode ??= "realtime";

		this.#encoderConfig = config;

		this.#encoder = new VideoEncoder({
			output: (frame, metadata) => {
				this.#encoded(frame, metadata);
			},
			error: (err) => {
				this.#decoder.reject(err);
				throw err;
			},
		});

		this.#encoder.configure(this.#encoderConfig);
	}

	static async isSupported(config: VideoEncoderConfig) {
		// Check if we support a specific codec family
		const short = config.codec.substring(0, 4);
		if (!SUPPORTED.includes(short)) return false;

		// Default to hardware encoding
		config.hardwareAcceleration ??= "prefer-hardware";

		// Default to CBR
		config.bitrateMode ??= "constant";

		// Default to realtime encoding
		config.latencyMode ??= "realtime";

		const res = await VideoEncoder.isConfigSupported(config);
		return !!res.supported;
	}

	get config() {
		return this.#encoderConfig;
	}

	async decoder(): Promise<VideoDecoderConfig> {
		return await this.#decoder.promise;
	}

	async run(context: Context, media: MediaStreamVideoTrack) {
		const input = new MediaStreamTrackProcessor({ track: media });
		const reader = input.readable.getReader();

		for (;;) {
			const { done, value } = await Promise.any([reader.read(), context.done]);
			if (done) {
				break;
			}

			// Set keyFrame to undefined when we're not sure so the encoder can decide.
			this.#encoder.encode(value, { keyFrame: this.#keyframeNext });
			this.#keyframeNext = undefined;
			value.close();
		}
	}

	async #encoded(frame: EncodedVideoChunk, metadata?: EncodedVideoChunkMetadata) {
		if (this.#decoder.pending) {
			const config = metadata?.decoderConfig;
			if (!config) throw new Error("missing decoder config");
			this.#decoder.resolve(config);
		}

		if (frame.type === "key") {
			this.#keyframeCounter = 0;
			this.#outputGroup?.close();
			this.#outputGroup = this.#outputTrack.appendGroup();
		} else {
			this.#keyframeCounter += 1;
			const framesPerGop = this.#encoderConfig.framerate ? 2 * this.#encoderConfig.framerate : 60;
			if (this.#keyframeCounter + this.#encoder.encodeQueueSize >= framesPerGop) {
				this.#keyframeNext = true;
			}
		}

		if (!this.#outputGroup) throw new Error("missing keyframe");

		const buffer = new Uint8Array(frame.byteLength);
		frame.copyTo(buffer);

		const karp = new Frame(frame.type, frame.timestamp, buffer);
		karp.encode(this.#outputGroup);
	}
}
