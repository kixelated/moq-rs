import * as Moq from "@kixelated/moq";
import type * as Catalog from "../media";
import * as Media from "../media";

// A map of sample rate to AudioContext.
// This super weird, but means we get automatic audio sample rate conversion.
// TODO add a way to close these.
const contexts = new Map<number, AudioContext>();

export class Audio {
	#volume = 0.5;
	#paused = false;

	#track?: AudioTrack;

	// Save these variables so we can reload the video if `visible` or `canvas` changes.
	#broadcast?: Moq.BroadcastReader;
	#tracks?: Catalog.Audio[];

	get volume(): number {
		return this.#volume;
	}

	set volume(volume: number) {
		this.#volume = volume;
	}

	get paused(): boolean {
		return this.#paused;
	}

	set paused(paused: boolean) {
		this.#paused = paused;
		this.#reload();
	}

	load(broadcast: Moq.BroadcastReader, tracks: Catalog.Audio[]) {
		this.#broadcast?.close();
		this.#broadcast = broadcast;
		this.#tracks = tracks;

		this.#reload();
	}

	close() {
		this.#broadcast?.close();
		this.#broadcast = undefined;
		this.#tracks = undefined;
		this.#track?.close();
	}

	async init() {
		for (const context of contexts.values()) {
			await context.resume();
		}

		this.#reload();
	}

	#reload() {
		const info = this.#tracks?.at(0);
		if (info && this.#track?.info === info) {
			// No change; continue the current subscription.
			return;
		}

		this.#track?.close();
		this.#track = undefined;

		if (!this.#tracks || !this.#broadcast || !info || this.#paused) {
			// Nothing to render, unsubscribe.
			return;
		}

		let context = contexts.get(info.sample_rate);
		if (!context) {
			context = new AudioContext({ latencyHint: "interactive", sampleRate: info.sample_rate });
			contexts.set(info.sample_rate, context);
		}

		if (context.state !== "running") {
			// Audio is disabled because we haven't clicked yet.
			return;
		}

		const track = this.#broadcast.subscribe(info.track.name, info.track.priority);
		this.#track = new AudioTrack(info, track, context);
	}
}

export class AudioTrack {
	info: Catalog.Audio;

	#context: AudioContext;
	#container: Media.Reader;
	#decoder: AudioDecoder;

	#active?: number;
	#queued?: number;

	#ref?: number;

	constructor(info: Catalog.Audio, track: Moq.TrackReader, context: AudioContext) {
		this.info = info;
		this.#context = context;
		this.#decoder = new AudioDecoder({
			output: (data) => this.#decoded(data),
			error: (error) => {
				console.error(error);
				this.close();
			},
		});

		this.#decoder.configure({
			codec: info.codec,
			sampleRate: info.sample_rate,
			numberOfChannels: info.channel_count,
		});

		this.#container = new Media.Reader(track);

		this.#run().finally(() => {
			this.close();
		});
	}

	#decoded(data: AudioData) {
		if (this.#queued) {
			data.close();
			return;
		}

		const diff = data.timestamp / 1_000_000 - this.#context.currentTime;
		if (!this.#ref) {
			this.#ref = diff;
		} else {
			//console.log(Math.round((diff - this.#ref) * 1000));
		}

		const buffer = this.#context.createBuffer(data.numberOfChannels, data.numberOfFrames, data.sampleRate);

		for (let channel = 0; channel < data.numberOfChannels; channel++) {
			const channelData = new Float32Array(data.numberOfFrames);
			data.copyTo(channelData, { format: "f32-planar", planeIndex: channel });
			buffer.copyToChannel(channelData, channel);
		}
		data.close();

		const source = this.#context.createBufferSource();
		source.buffer = buffer;
		source.connect(this.#context.destination);
		source.onended = () => this.#advance();

		if (this.#active) {
			source.start(this.#active);
			this.#queued = this.#active + buffer.duration;
		} else {
			source.start();
			this.#active = this.#context.currentTime + buffer.duration;
		}
	}

	#advance() {
		this.#active = this.#queued;
		this.#queued = undefined;
	}

	async #run() {
		for (;;) {
			const frame = await this.#container.readFrame();
			if (!frame) break;

			const chunk = new EncodedAudioChunk({
				type: "key",
				data: frame.data,
				timestamp: frame.timestamp,
			});

			this.#decoder.decode(chunk);
		}
	}

	close() {
		try {
			this.#container.close();
			this.#decoder.close();
		} catch (error) {
			// Don't care
		}
	}
}
