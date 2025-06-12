import * as Moq from "@kixelated/moq";
import { Memo, Signal, Signals, signal } from "@kixelated/signals";
import * as Catalog from "../catalog";
import * as Container from "../container";

// An annoying hack, but there's stuttering that we need to fix.
const LATENCY = 50;

// Expose the root of the audio graph so we can use it for custom visualizations.
export type AudioRoot = {
	context: AudioContext;
	node: GainNode;
};

export type AudioEmitterProps = {
	volume?: number;
	muted?: boolean;
	paused?: boolean;
};

// A helper that emits audio directly to the speakers.
export class AudioEmitter {
	source: Audio;
	volume: Signal<number>;
	muted: Signal<boolean>;

	// Similar to muted, but controls whether we download audio at all.
	// That way we can be "muted" but also download audio for visualizations.
	paused: Signal<boolean>;

	#signals = new Signals();

	// The volume to use when unmuted.
	#unmuteVolume = 0.5;

	// The gain node used to adjust the volume.
	#gain = signal<GainNode | undefined>(undefined);

	constructor(source: Audio, props?: AudioEmitterProps) {
		this.source = source;
		this.volume = signal(props?.volume ?? 0.5);
		this.muted = signal(props?.muted ?? false);
		this.paused = signal(props?.paused ?? props?.muted ?? false);

		// Set the volume to 0 when muted.
		this.#signals.effect(() => {
			const muted = this.muted.get();
			if (muted) {
				this.#unmuteVolume = this.volume.peek() || 0.5;
				this.volume.set(0);
				this.source.enabled.set(false);
			} else {
				this.volume.set(this.#unmuteVolume);
				this.source.enabled.set(true);
			}
		});

		// Set unmute when the volume is non-zero.
		this.#signals.effect(() => {
			const volume = this.volume.get();
			this.muted.set(volume === 0);
		});

		this.#signals.effect(() => {
			const root = this.source.root.get();
			if (!root) return;

			const gain = new GainNode(root.context, { gain: this.volume.peek() });
			root.node.connect(gain);
			gain.connect(root.context.destination); // speakers

			this.#gain.set(gain);
		});

		this.#signals.effect(() => {
			const gain = this.#gain.get();
			if (!gain) return;

			const volume = this.volume.get();
			gain.gain.value = volume;
		});

		this.#signals.effect(() => {
			this.source.enabled.set(!this.paused.get());
		});
	}

	close() {
		this.#signals.close();
	}
}

export type AudioProps = {
	enabled?: boolean;
};

// Downloads audio from a track and emits it to an AudioContext.
// The user is responsible for hooking up audio to speakers, an analyzer, etc.
export class Audio {
	broadcast: Signal<Moq.BroadcastConsumer | undefined>;
	catalog: Signal<Catalog.Root | undefined>;
	enabled: Signal<boolean>;
	selected: Memo<Catalog.Audio | undefined>;

	// The raw audio context, which can be used for custom visualizations.
	// If using this class directly, you'll need to hook up audio to speakers, an analyzer, etc.
	#root = signal<AudioRoot | undefined>(undefined);
	readonly root = this.#root.readonly();

	#sampleRate = signal<number | undefined>(undefined);
	readonly sampleRate = this.#sampleRate.readonly();

	// Reusable audio buffers.
	#buffers: AudioBuffer[] = [];
	#active: { node: AudioBufferSourceNode; timestamp: number }[] = [];

	// Used to convert from timestamp units to AudioContext units.
	#ref?: number;

	#signals = new Signals();

	constructor(
		broadcast: Signal<Moq.BroadcastConsumer | undefined>,
		catalog: Signal<Catalog.Root | undefined>,
		props?: AudioProps,
	) {
		this.broadcast = broadcast;
		this.catalog = catalog;
		this.enabled = signal(props?.enabled ?? false);

		this.selected = this.#signals.memo(() => this.catalog.get()?.audio?.[0], { deepEquals: true });

		// Stop all active samples when disabled.
		this.#signals.effect(() => {
			const enabled = this.enabled.get();
			if (enabled) return;

			for (const active of this.#active) {
				active.node.stop();
			}
		});

		this.#signals.effect(() => {
			const enabled = this.enabled.get();
			if (!enabled) return undefined;

			const sampleRate = this.#sampleRate.get();
			if (!sampleRate) return undefined;

			// NOTE: We still create an AudioContext even when muted.
			// This way we can process the audio for visualizations.

			const root = new AudioContext({ latencyHint: "interactive", sampleRate });
			if (root.state === "suspended") {
				// Force disabled if autoplay restrictions are preventing us from playing.
				this.enabled.set(false);
				return;
			}

			// Make a dummy gain node that we can expose.
			const node = new GainNode(root, { gain: 1 });
			this.#root.set({ context: root, node });

			return () => {
				node.disconnect();
				root.close();
				this.#root.set(undefined);
			};
		});

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
			output: (data) => this.#emit(data),
			error: (error) => console.error(error),
		});

		const config = selected.config;

		decoder.configure({
			...config,
			description: config.description ? Buffer.from(config.description, "hex") : undefined,
		});

		(async () => {
			for (;;) {
				const frame = await sub.nextFrame();
				if (!frame) break;

				const decoded = Container.decodeFrame(frame.data);

				const chunk = new EncodedAudioChunk({
					type: "key",
					data: decoded.data,
					timestamp: decoded.timestamp,
				});

				decoder.decode(chunk);
			}
		})();

		return () => {
			sub.close();
			decoder.close();
		};
	}

	#emit(sample: AudioData) {
		this.#sampleRate.set(sample.sampleRate);

		const root = this.#root.get();
		if (!root) {
			sample.close();
			return;
		}

		// Convert from microseconds to seconds.
		const timestamp = sample.timestamp / 1_000_000;

		// The maximum latency in seconds, including a full frame size.
		const maxLatency = sample.numberOfFrames / sample.sampleRate + LATENCY / 1000;

		if (!this.#ref) {
			this.#ref = timestamp - root.context.currentTime - maxLatency;
		}

		// Determine when the sample should be played in AudioContext units.
		let when = timestamp - this.#ref;
		const latency = when - root.context.currentTime;
		if (latency < 0) {
			// Can't play in the past.
			sample.close();
			return;
		}

		if (latency > maxLatency) {
			// We went over the max latency, so we need a new ref.
			this.#ref = timestamp - root.context.currentTime - maxLatency;

			// Cancel any active samples and let them reschedule themselves if needed.
			for (const active of this.#active) {
				active.node.stop();
			}

			// Schedule the sample to play at the max latency.
			when = root.context.currentTime + maxLatency;
		}

		// Create an audio buffer for this sample.
		const buffer = this.#createBuffer(sample, root.context);
		this.#scheduleBuffer(root, buffer, timestamp, when);
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

	#scheduleBuffer(root: AudioRoot, buffer: AudioBuffer, timestamp: number, when: number) {
		const source = root.context.createBufferSource();
		source.buffer = buffer;
		source.connect(root.node);
		source.onended = () => {
			// Remove ourselves from the active list.
			// This is super gross and probably wrong, but yolo.
			this.#active.shift();

			// Check if we need to reschedule this sample because it was cancelled.
			if (this.#ref) {
				const newWhen = timestamp - this.#ref;
				if (newWhen > root.context.currentTime) {
					// Reschedule the sample to play at the new time.
					this.#scheduleBuffer(root, buffer, timestamp, newWhen);
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
