import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import * as Container from "../container";
import { Derived, Signal, Signals, signal } from "../signals";

// An annoying hack, but there's stuttering that we need to fix.
const LATENCY = 50;

// A pair of AudioContext and GainNode.
export type WatchAudioContext = {
	root: AudioContext;
	gain: GainNode;
};

export type WatchAudioProps = {
	volume?: number;
	paused?: boolean;
	muted?: boolean;
};

export class WatchAudio {
	source: WatchAudioSource;
	volume: Signal<number>;
	muted: Signal<boolean>;
	paused: Signal<boolean>;

	#context = signal<WatchAudioContext | undefined>(undefined);
	readonly context = this.#context.readonly();

	#sampleRate = signal<number | undefined>(undefined);
	readonly sampleRate = this.#sampleRate.readonly();

	// Reusable audio buffers.
	#buffers: AudioBuffer[] = [];
	#active: { node: AudioBufferSourceNode; timestamp: number }[] = [];

	// Used to convert from timestamp units to AudioContext units.
	#ref?: number;
	#signals = new Signals();

	// The volume to use when unmuted.
	#unmuteVolume = 0.5;

	constructor(source: WatchAudioSource, props?: WatchAudioProps) {
		this.source = source;
		this.volume = signal(props?.volume ?? 0.5);
		this.muted = signal(props?.muted ?? true); // default to true because autoplay restrictions
		this.paused = signal(props?.paused ?? false);

		this.#signals.effect(() => {
			const ctx = this.#context.get();
			if (!ctx) return;

			const volume = this.volume.get();
			ctx.gain.gain.value = volume;

			if (volume === 0) {
				for (const active of this.#active) {
					active.node.stop();
				}
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

		// Set the volume to 0 when muted.
		this.#signals.effect(() => {
			const muted = this.muted.get();
			if (muted) {
				this.#unmuteVolume = this.volume.peek() || 0.5;
				this.volume.set(0);
			} else {
				this.volume.set(this.#unmuteVolume);
			}
		});

		// Set unmute when the volume is non-zero.
		this.#signals.effect(() => {
			const volume = this.volume.get();

			// Only download audio when the volume is non-zero.
			this.muted.set(volume === 0);
			this.source.enabled.set(volume > 0);
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
		const paused = this.paused.get();
		if (!context || paused) {
			sample.close();
			return;
		}

		// Convert from microseconds to seconds.
		const timestamp = sample.timestamp / 1_000_000;

		// The maximum latency in seconds, including a full frame size.
		const maxLatency = sample.numberOfFrames / sample.sampleRate + LATENCY / 1000;

		if (!this.#ref) {
			this.#ref = timestamp - context.root.currentTime - maxLatency;
		}

		// Determine when the sample should be played in AudioContext units.
		let when = timestamp - this.#ref;
		const latency = when - context.root.currentTime;
		if (latency < 0) {
			// Can't play in the past.
			sample.close();
			return;
		}

		if (latency > maxLatency) {
			// We went over the max latency, so we need a new ref.
			this.#ref = timestamp - context.root.currentTime - maxLatency;

			// Cancel any active samples and let them reschedule themselves if needed.
			for (const active of this.#active) {
				active.node.stop();
			}

			// Schedule the sample to play at the max latency.
			when = context.root.currentTime + maxLatency;
		}

		// Create an audio buffer for this sample.
		const buffer = this.#createBuffer(sample, context.root);
		this.#scheduleBuffer(context, buffer, timestamp, when);
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

	#scheduleBuffer(context: WatchAudioContext, buffer: AudioBuffer, timestamp: number, when: number) {
		const source = context.root.createBufferSource();
		source.buffer = buffer;
		source.connect(context.gain);
		source.onended = () => {
			// Remove ourselves from the active list.
			// This is super gross and probably wrong, but yolo.
			this.#active.shift();

			// Check if we need to reschedule this sample because it was cancelled.
			if (this.#ref) {
				const newWhen = timestamp - this.#ref;
				if (newWhen > context.root.currentTime) {
					// Reschedule the sample to play at the new time.
					this.#scheduleBuffer(context, buffer, timestamp, newWhen);
					return;
				}
			}

			this.#buffers.push(buffer);
		};
		source.start(when);

		this.#active.push({ node: source, timestamp });
	}

	close() {
		this.#signals.close();
	}
}

export type WatchAudioSourceProps = {
	broadcast?: Moq.BroadcastConsumer;
	available?: Catalog.Audio[];
	enabled?: boolean;
};

export class WatchAudioSource {
	broadcast: Signal<Moq.BroadcastConsumer | undefined>;
	available: Signal<Catalog.Audio[]>;
	enabled: Signal<boolean>;
	selected: Derived<Catalog.Audio | undefined>;

	samples: ReadableStream<AudioData>;
	#writer: WritableStreamDefaultWriter<AudioData>;

	// TODO add max bitrate/resolution/etc.

	#signals = new Signals();

	constructor(props?: WatchAudioSourceProps) {
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
			...selected,
			description: selected.description ? Buffer.from(selected.description, "hex") : undefined,
		});

		const media = new Container.Decoder(sub);

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
