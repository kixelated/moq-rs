import * as Moq from "@kixelated/moq";
import type * as Catalog from "../media";
import * as Media from "../media";

// Responsible for choosing the best audio track for an active broadcast.
export class AudioTracks {
	broadcast: Moq.BroadcastReader;
	#tracks: Media.Audio[] = [];
	#active?: AudioTrack;

	// The combined audio samples from all tracks.
	samples: ReadableStream<AudioData>;
	#writer: WritableStream<AudioData>;

	// constraints
	// Audio starts disabled by default.
	// This is not really by choice; browsers require user interaction before audio contexts can be created.
	#enabled = false;

	constructor(broadcast: Moq.BroadcastReader) {
		this.broadcast = broadcast;

		const queue = new TransformStream<AudioData, AudioData>();
		this.#writer = queue.writable;
		this.samples = queue.readable;
	}

	get tracks(): Media.Audio[] {
		return this.#tracks;
	}

	set tracks(tracks: Media.Audio[]) {
		this.#tracks = tracks;
		this.#reload();
	}

	get enabled(): boolean {
		return this.#enabled;
	}

	set enabled(enabled: boolean) {
		this.#enabled = enabled;
		this.#reload();
	}

	// TODO add other constraints.

	#reload() {
		if (!this.#enabled) {
			this.#active?.close();
			this.#active = undefined;
			return;
		}

		const info = this.#tracks.at(0);
		if (this.#active?.info === info) {
			return;
		}

		this.#active?.close();
		this.#active = undefined;

		if (info) {
			const track = this.broadcast.subscribe(info.track.name, info.track.priority);
			this.#active = new AudioTrack(track, info);

			this.#active.samples.pipeTo(this.#writer, { preventClose: true, preventCancel: true }).catch(() => void 0);
		}
	}

	close() {
		this.broadcast.close();
		this.#active?.close();
	}
}

// A single audio track, decoding and writing audio samples to a stream.
export class AudioTrack {
	info: Catalog.Audio;
	samples: ReadableStream<AudioData>;
	#writer: WritableStreamDefaultWriter<AudioData>;

	#container: Media.Reader;
	#decoder: AudioDecoder;

	constructor(track: Moq.TrackReader, info: Catalog.Audio) {
		this.info = info;

		const queue = new TransformStream<AudioData, AudioData>();
		this.#writer = queue.writable.getWriter();
		this.samples = queue.readable;

		this.#decoder = new AudioDecoder({
			output: (data) => this.#writer.write(data),
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
		this.#container.close();
		this.#writer.close().catch(() => void 0);

		try {
			this.#decoder.close();
		} catch (error) {
			// Don't care
		}
	}
}

// A pair of AudioContext and GainNode.
type Context = {
	root: AudioContext;
	gain: GainNode;
};

export class AudioEmitter {
	#context?: Context;

	#volume = 0.5;
	#paused = true;

	#broadcast?: AudioTracks;

	#writer: WritableStream<AudioData>;
	#reader: ReadableStreamDefaultReader<AudioData>;

	// Reusable audio buffers.
	#buffers: AudioBuffer[] = [];

	#active: AudioBufferSourceNode[] = [];

	// The maximum latency in seconds.
	// The larger the value, the more tolerance we have for network jitter.
	// Audio gaps are more noticable so a larger buffer is recommended.
	#maxLatency = 50;

	// Used to convert from timestamp units to AudioContext units.
	#ref?: number;

	// A callback that allows connecting AudioNodes to the AudioContext.
	// An AudioContext will be created based on the sampleRate.
	// By default, we just output to the speakers.
	onInit: (ctx: AudioContext, dst: AudioNode) => void = (ctx, dst) => {
		dst.connect(ctx.destination);
	};

	constructor() {
		const queue = new TransformStream<AudioData, AudioData>();
		this.#writer = queue.writable;
		this.#reader = queue.readable.getReader();

		this.#run();
	}

	get broadcast(): AudioTracks | undefined {
		return this.#broadcast;
	}

	set broadcast(broadcast: AudioTracks | undefined) {
		this.#broadcast?.close();

		if (broadcast) {
			broadcast.enabled = !this.#paused && this.#volume > 0;
			broadcast.samples.pipeTo(this.#writer, { preventClose: true, preventCancel: true }).catch(() => void 0);
		}

		this.#broadcast = broadcast;
	}

	get volume(): number {
		return this.#volume;
	}

	set volume(volume: number) {
		this.#volume = volume;

		if (this.#context) {
			this.#context.gain.gain.value = this.#volume;
		}

		if (this.#broadcast) {
			this.#broadcast.enabled = !this.#paused && this.#volume > 0;
		}
	}

	get muted(): boolean {
		return this.#paused;
	}

	set muted(paused: boolean) {
		this.#paused = paused;

		if (this.#broadcast) {
			this.#broadcast.enabled = !this.#paused && this.#volume > 0;
		}
	}

	get latency(): DOMHighResTimeStamp {
		return this.#maxLatency;
	}

	set latency(maxLatency: DOMHighResTimeStamp) {
		this.#maxLatency = maxLatency;
		this.#ref = undefined;
	}

	async #run() {
		for (;;) {
			const { done, value: sample } = await this.#reader.read();
			if (done) break;

			this.#emit(sample);
		}
	}

	#emit(sample: AudioData) {
		const context = this.#init(sample.sampleRate);

		// Convert from microseconds to seconds.
		const timestamp = sample.timestamp / 1_000_000;
		const maxLatency = this.#maxLatency / 1000;

		if (!this.#ref) {
			this.#ref = timestamp - context.root.currentTime - maxLatency;
		}

		// Determine when the sample should be played in AudioContext units.
		const when = timestamp - this.#ref;
		const latency = when - context.root.currentTime;

		if (latency > maxLatency) {
			// Skip this sample if it's too far in the future.
			this.#ref += sample.numberOfFrames / sample.sampleRate;
			sample.close();
			return;
		}

		// Create an audio buffer for this sample.
		const buffer = this.#createBuffer(sample, context.root);

		const source = context.root.createBufferSource();
		source.buffer = buffer;
		source.connect(context.gain);
		source.onended = () => {
			this.#buffers.push(buffer);
			this.#active.shift();
		};
		source.start(when);

		this.#active.push(source);
	}

	#init(sampleRate: number): Context {
		if (this.#context?.root.sampleRate === sampleRate) {
			if (this.#context.root.state === "suspended") {
				this.#context.root.resume();
			}

			return this.#context;
		}

		if (this.#context) {
			this.#context.root.close();
		}

		this.#ref = undefined;

		const root = new AudioContext({ latencyHint: "interactive", sampleRate });
		const gain = new GainNode(root, { gain: this.#volume });

		this.#context = { root, gain };
		this.onInit(this.#context.root, this.#context.gain);

		return this.#context;
	}

	#createBuffer(sample: AudioData, context: AudioContext): AudioBuffer {
		let buffer: AudioBuffer | undefined;

		while (this.#buffers.length > 0) {
			const reuse = this.#buffers.shift();
			if (
				reuse &&
				reuse.sampleRate === sample.sampleRate &&
				reuse.numberOfChannels === sample.numberOfChannels &&
				reuse.length === sample.numberOfFrames
			) {
				buffer = reuse;
				break;
			}
		}

		if (!buffer) {
			buffer = context.createBuffer(sample.numberOfChannels, sample.numberOfFrames, sample.sampleRate);
		}

		// Copy the sample data to the buffer.
		for (let channel = 0; channel < sample.numberOfChannels; channel++) {
			const channelData = new Float32Array(sample.numberOfFrames);
			sample.copyTo(channelData, { format: "f32-planar", planeIndex: channel });
			buffer.copyToChannel(channelData, channel);
		}
		sample.close();

		return buffer;
	}

	close() {
		this.#context?.root.close();
		this.#writer.close().catch(() => void 0);
		this.#reader.cancel().catch(() => void 0);
	}
}
