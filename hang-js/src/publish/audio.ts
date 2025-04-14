import { Frame } from "../container/frame";
import { Deferred } from "../util/async";

import * as Moq from "@kixelated/moq";
import { Context } from "../util/context";

const SUPPORTED = [
	// TODO support AAC
	// "mp4a"
	"Opus",
];

export type EncoderConfig = AudioEncoderConfig;

export class Encoder {
	#output: Moq.TrackWriter;

	#encoder: AudioEncoder;
	#encoderConfig: AudioEncoderConfig;
	#decoder: AudioDecoderConfig;

	constructor(config: AudioEncoderConfig, output: Moq.TrackWriter) {
		this.#output = output;

		this.#encoderConfig = config;
		this.#decoder = {
			codec: config.codec,
			numberOfChannels: config.numberOfChannels,
			sampleRate: config.sampleRate,
		};

		this.#encoder = new AudioEncoder({
			output: (frame, metadata) => this.#encoded(frame, metadata),
			error: (err) => {
				throw err;
			},
		});

		this.#encoder.configure(this.#encoderConfig);
	}

	get config() {
		return this.#encoderConfig;
	}

	decoder(): AudioDecoderConfig {
		return this.#decoder;
	}

	async run(context: Context, media: MediaStreamAudioTrack) {
		const input = new MediaStreamTrackProcessor({ track: media });
		const reader = input.readable.getReader();

		for (;;) {
			const result = await context.race(reader.read());
			if (!result || result.done) {
				break;
			}

			const frame = result.value;
			this.#encoder.encode(frame);
			frame.close();
		}
	}

	#encoded(frame: EncodedAudioChunk, metadata?: EncodedAudioChunkMetadata) {
		if (frame.type !== "key") {
			throw new Error("only key frames are supported");
		}

		const group = this.#output.appendGroup();

		const buffer = new Uint8Array(frame.byteLength);
		frame.copyTo(buffer);

		const hang = new Frame(frame.type === "key", frame.timestamp, buffer);
		hang.encode(group);

		group.close();
	}

	static async isSupported(config: AudioEncoderConfig) {
		// Check if we support a specific codec family
		const short = config.codec.substring(0, 4);
		if (!SUPPORTED.includes(short)) return false;

		const res = await AudioEncoder.isConfigSupported(config);
		return !!res.supported;
	}
}
