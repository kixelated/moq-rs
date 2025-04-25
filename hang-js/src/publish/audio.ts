import { Frame } from "../container/frame";

import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import { AudioTrackSettings } from "../util/settings";

const SUPPORTED = [
	// TODO support AAC
	// "mp4a"
	"Opus",
];

export class Audio {
	readonly media: MediaStreamAudioTrack;
	readonly catalog: Catalog.Audio;
	readonly settings: AudioTrackSettings;
	readonly track: Moq.Track;

	#encoder: AudioEncoder;

	constructor(media: MediaStreamAudioTrack) {
		this.media = media;
		this.track = new Moq.Track("audio", 1);
		this.settings = media.getSettings() as AudioTrackSettings;

		this.catalog = {
			track: {
				name: "audio",
				priority: 1,
			},
			codec: "Opus",
			sample_rate: this.settings.sampleRate,
			channel_count: this.settings.channelCount,
		};

		this.#encoder = new AudioEncoder({
			output: (frame, metadata) => this.#encoded(frame, metadata),
			error: (err) => this.track.writer.abort(err),
		});

		this.#encoder.configure({
			codec: this.catalog.codec,
			numberOfChannels: this.catalog.channel_count,
			sampleRate: this.catalog.sample_rate,
		});

		this.#run()
			.catch((err) => this.track.writer.abort(err))
			.finally(() => this.track.writer.close());
	}

	async #run() {
		const input = new MediaStreamTrackProcessor({ track: this.media });
		const reader = input.readable.getReader();

		for (;;) {
			const result = await reader.read();
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

		// TODO align with video
		const group = this.track.writer.appendGroup();

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

	close() {
		this.track.writer.close();
		this.#encoder.close();
	}
}
