import * as Moq from "@kixelated/moq";
import * as Media from "../media";
import { Derived, Signal, Signals, signal } from "../signals";

export type AudioProps = {
	broadcast?: Moq.BroadcastReader;
	available?: Media.Audio[];
	enabled?: boolean;
};

export class Audio {
	broadcast: Signal<Moq.BroadcastReader | undefined>;
	available: Signal<Media.Audio[]>;
	enabled: Signal<boolean>;
	selected: Derived<Media.Audio | undefined>;

	samples: ReadableStream<AudioData>;
	#writer: WritableStreamDefaultWriter<AudioData>;

	// TODO add max bitrate/resolution/etc.

	#signals = new Signals();

	constructor(props?: AudioProps) {
		this.broadcast = signal(props?.broadcast);
		this.available = signal(props?.available ?? []);
		this.enabled = signal(props?.enabled ?? false);

		const queue = new TransformStream<AudioData, AudioData>();
		this.#writer = queue.writable.getWriter();
		this.samples = queue.readable;

		this.selected = this.#signals.derived(() => this.available.get().at(0));
		this.#signals.effect(() => this.#init());
	}

	#init() {
		if (!this.enabled.get()) return;

		const selected = this.selected.get();
		if (!selected) return;

		const broadcast = this.broadcast.get();
		if (!broadcast) return;

		const sub = broadcast.subscribe(selected.track.name, selected.track.priority);

		const decoder = new AudioDecoder({
			output: (data) => this.#writer.write(data),
			error: (error) => console.error(error),
		});

		decoder.configure({
			codec: selected.codec,
			sampleRate: selected.sample_rate,
			numberOfChannels: selected.channel_count,
		});

		const media = new Media.Reader(sub);

		(async () => {
			for (;;) {
				const frame = await media.readFrame();
				if (!frame) break;

				const chunk = new EncodedAudioChunk({
					type: "key",
					data: frame.data,
					timestamp: frame.timestamp,
				});

				decoder.decode(chunk);
			}
		})();

		return () => {
			sub.close();
			media.close();
			decoder.close();
		};
	}

	close() {
		this.#signals.close();
		this.#writer.close().catch(() => void 0);
	}
}

// A pair of AudioContext and GainNode.
export type Context = {
	root: AudioContext;
	gain: GainNode;
};

export type AudioEmitterProps = {
	source: Audio;
	volume?: number;
	latency?: number;
	paused?: boolean;
};

export class AudioEmitter {
	source: Audio;
	volume: Signal<number>;
	paused: Signal<boolean>;

	// The maximum latency in seconds.
	// The larger the value, the more tolerance we have for network jitter.
	// Audio gaps are more noticable so a larger buffer is recommended.
	latency: Signal<number>;

	#context = signal<Context | undefined>(undefined);
	readonly context = this.#context.readonly();

	#sampleRate = signal<number | undefined>(undefined);
	readonly sampleRate = this.#sampleRate.readonly();

	// Reusable audio buffers.
	#buffers: AudioBuffer[] = [];
	#active: AudioBufferSourceNode[] = [];

	// Used to convert from timestamp units to AudioContext units.
	#ref?: number;
	#signals = new Signals();

	constructor(props: AudioEmitterProps) {
		this.source = props.source;
		this.volume = signal(props.volume ?? 0.5);
		this.latency = signal(props.latency ?? 50);
		this.paused = signal(props.paused ?? false);

		this.#signals.effect(() => {
			const ctx = this.#context.get();
			if (!ctx) return;

			const volume = this.volume.get();
			ctx.gain.gain.value = volume;

			if (volume === 0) {
				for (const active of this.#active) {
					active.stop();
				}
			}
		});

		// Update the ref when the latency changes.
		this.#signals.effect(() => {
			this.latency.get();
			this.#ref = undefined;
		});

		this.#signals.effect(() => {
			if (!this.paused.get()) return;

			// Cancel any active samples.
			for (const active of this.#active) {
				active.stop();
			}
		});

		this.#signals.effect(() => {
			const sampleRate = this.#sampleRate.get();
			if (!sampleRate) return undefined;

			const root = new AudioContext({ latencyHint: "interactive", sampleRate });
			const gain = new GainNode(root, { gain: this.volume.peek() });

			gain.connect(root.destination);

			this.#context.set({ root, gain });

			return () => {
				gain.disconnect();
				root.close();
			};
		});

		this.#run();
	}

	async #run() {
		const reader = this.source.samples.getReader();
		for (;;) {
			const { value: sample } = await reader.read();
			if (!sample) break;

			this.#emit(sample);
		}

		reader.releaseLock();
	}

	#emit(sample: AudioData) {
		this.#sampleRate.set(sample.sampleRate);

		const context = this.#context.get();
		if (!context) return;

		if (this.paused.get()) return;

		// Convert from microseconds to seconds.
		const timestamp = sample.timestamp / 1_000_000;
		const maxLatency = this.latency.get() / 1000;

		if (!this.#ref) {
			this.#ref = timestamp - context.root.currentTime - maxLatency;
		}

		// Determine when the sample should be played in AudioContext units.
		let when = timestamp - this.#ref;

		// Don't play samples in the past u nerd.
		// TODO This indicates a bug in the latency calculation TBH.
		if (when < 0) {
			this.#ref = timestamp;
			when = 0;
		}

		const latency = when - context.root.currentTime;

		if (latency > maxLatency) {
			// Cancel any active samples.
			for (const active of this.#active) {
				active.stop();
			}

			// Skip this sample if it's too far in the future.
			this.#ref += sample.numberOfFrames / sample.sampleRate;
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
		this.#signals.close();
	}
}
