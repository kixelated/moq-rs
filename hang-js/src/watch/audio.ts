import type * as Catalog from "../catalog";
import * as Moq from "@kixelated/moq";
import * as Container from "../container";

export class Audio {
	#volume = 0.5;
	#paused = false;

	#track?: AudioTrack;

	// Save these variables so we can reload the video if `visible` or `canvas` changes.
	#connection?: Moq.Connection;
	#broadcast?: string;
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
		this.#maybeReload();
	}

	load(connection: Moq.Connection, broadcast: string, tracks: Catalog.Audio[]) {
		this.#connection = connection;
		this.#broadcast = broadcast;
		this.#tracks = tracks;

		this.#maybeReload();
	}

	close() {
		this.#connection = undefined;
		this.#broadcast = undefined;
		this.#tracks = undefined;
		this.#track?.close();
	}

	reload() {
		this.#track?.close();
		this.#track = undefined;

		const info = this.#tracks?.at(0);

		if (!this.#tracks || !this.#connection || !this.#broadcast || !info || this.#paused) {
			// Nothing to render, unsubscribe.
			return;
		}

		const track = this.#connection.subscribe(this.#broadcast, info.track.name, info.track.priority);
		this.#track = new AudioTrack(info, track);
	}

	#maybeReload() {
		const info = this.#tracks?.at(0);
		if (this.#track && info && this.#track.info === info) {
			// No change; continue the current run.
			return;
		}

		this.reload();
	}
}

export class AudioTrack {
	info: Catalog.Audio;

	// TODO reuse AudioContext based on sample rate
	#context: AudioContext;

	#container: Container.Reader;
	#decoder: AudioDecoder;

	#active?: number;
	#queued?: number;

	#ref?: number;

	constructor(info: Catalog.Audio, track: Moq.TrackReader) {
		this.info = info;

		// TODO reuse AudioContext based on sample rate
		this.#context = new AudioContext({ latencyHint: "interactive", sampleRate: info.sample_rate });

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

		this.#container = new Container.Reader(track);

		this.#run();
	}

	#decoded(data: AudioData) {
		if (this.#queued) {
			data.close();
			return;
		}

		const diff = data.timestamp / 1000_000 - this.#context.currentTime;
		if (!this.#ref) {
			this.#ref = diff;
		} else {
			console.log(diff - this.#ref);
		}

		const buffer = this.#context.createBuffer(data.numberOfChannels, data.numberOfFrames, data.sampleRate);

		for (let channel = 0; channel < data.numberOfChannels; channel++) {
			const channelData = new Float32Array(data.numberOfFrames);
			data.copyTo(channelData, { planeIndex: channel });
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

		this.#decoder.close();
	}

	close() {
		this.#context.close();
		this.#container.close();
	}
}
