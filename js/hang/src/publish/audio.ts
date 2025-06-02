import * as Moq from "@kixelated/moq";
import { Signal, Signals, cleanup, signal } from "@kixelated/signals";
import * as Catalog from "../catalog";
import { Frame } from "../container/frame";

// Create a group every half a second
const GOP_DURATION = 0.5;

export type AudioConstraints = Omit<
	MediaTrackConstraints,
	"aspectRatio" | "backgroundBlur" | "displaySurface" | "facingMode" | "frameRate" | "height" | "width"
>;

// Stronger typing for the MediaStreamTrack interface.
export interface AudioTrack extends MediaStreamTrack {
	kind: "audio";
	clone(): AudioTrack;
}

// MediaTrackSettings can represent both audio and video, which means a LOT of possibly undefined properties.
// This is a fork of the MediaTrackSettings interface with properties required for audio or vidfeo.
export interface AudioTrackSettings {
	deviceId: string;
	groupId: string;

	autoGainControl: boolean;
	channelCount: number;
	echoCancellation: boolean;
	noiseSuppression: boolean;
	sampleRate: number;
	sampleSize: number;
}

export type AudioProps = {
	media?: AudioTrack;
	constraints?: AudioConstraints | boolean;
};

export class Audio {
	broadcast: Moq.BroadcastProducer;

	readonly media: Signal<AudioTrack | undefined>;
	readonly constraints: Signal<AudioConstraints | boolean | undefined>;

	#catalog = signal<Catalog.Audio | undefined>(undefined);
	readonly catalog = this.#catalog.readonly();

	// Expose the AudioContext and AudioNode for any audio renderer.
	#root = signal<{ context: AudioContext; node: AudioWorkletNode } | undefined>(undefined);
	readonly root = this.#root.readonly();

	#group?: Moq.GroupProducer;
	#groupTimestamp = 0;

	#id = 0;
	#signals = new Signals();

	constructor(broadcast: Moq.BroadcastProducer, props?: AudioProps) {
		this.broadcast = broadcast;
		this.media = signal(props?.media);
		this.constraints = signal(props?.constraints);

		this.#signals.effect(() => this.#runCatalog());
		this.#signals.effect(() => this.#runWorklet());
		this.#signals.effect(() => this.#runEncoder());
	}

	#runCatalog() {
		const media = this.media.get();
		if (!media) return;
	}

	#runWorklet() {
		const media = this.media.get();
		if (!media) return;

		const settings = media.getSettings();
		if (!settings) {
			throw new Error("track has no settings");
		}

		const root = this.root.get();
		if (!root) return;

		const context = new AudioContext({
			sampleRate: settings.sampleRate,
		});

		// Async because we need to wait for the worklet to be registered.
		// Annoying, I know
		root.context.audioWorklet.addModule(`data:text/javascript,(${worklet.toString()})()`).then(() => {
			const node = new AudioWorkletNode(root.context, "capture");
			this.#root.set({ context, node });
		});

		return () => {
			this.#root.set((p) => {
				p?.node.disconnect();
				p?.context.close();
				return undefined;
			});
		};
	}

	#runEncoder() {
		const root = this.root.get();
		if (!root) return;

		const media = this.media.get();
		if (!media) return;

		const { context, node } = root;

		const track = new Moq.TrackProducer(`audio-${this.#id++}`, 1);
		this.broadcast.insertTrack(track.consume());

		cleanup(() => track.close());
		cleanup(() => this.broadcast.removeTrack(track.name));

		const settings = media.getSettings() as AudioTrackSettings;

		// TODO: This is a Firefox hack to get the sample rate.
		const sampleRate =
			settings.sampleRate ??
			(() => {
				const ctx = new AudioContext();
				const rate = ctx.sampleRate;
				ctx.close();
				return rate;
			})();

		const catalog = {
			track: {
				name: track.name,
				priority: track.priority,
			},
			config: {
				// TODO get codec and description from decoderConfig
				codec: "opus",
				sampleRate,
				numberOfChannels: settings.channelCount,
				// TODO configurable
				bitrate: 64_000,
			},
		};

		this.#catalog.set(catalog);
		cleanup(() => this.#catalog.set(undefined));

		const encoder = new AudioEncoder({
			output: (frame) => {
				if (frame.type !== "key") {
					throw new Error("only key frames are supported");
				}

				if (!this.#group || frame.timestamp - this.#groupTimestamp >= 1000 * 1000 * GOP_DURATION) {
					this.#group?.close();
					this.#group = track.appendGroup();
					this.#groupTimestamp = frame.timestamp;
				}

				const buffer = new Uint8Array(frame.byteLength);
				frame.copyTo(buffer);

				const hang = new Frame(frame.type === "key", frame.timestamp, buffer);
				hang.encode(this.#group);
			},
			error: (err) => {
				this.#group?.abort(err);
				this.#group = undefined;

				track.abort(err);
			},
		});
		cleanup(() => encoder.close());

		const config = catalog.config;

		encoder.configure({
			codec: config.codec,
			numberOfChannels: config.numberOfChannels,
			sampleRate: config.sampleRate,
			bitrate: config.bitrate,
		});

		node.port.addEventListener("message", ({ data: channels }: { data: Float32Array[] }) => {
			const joinedLength = channels.reduce((a, b) => a + b.length, 0);
			const joined = new Float32Array(joinedLength);

			channels.reduce((offset: number, channel: Float32Array): number => {
				joined.set(channel, offset);
				return offset + channel.length;
			}, 0);

			const frame = new AudioData({
				format: "f32-planar",
				sampleRate: context.sampleRate,
				numberOfFrames: channels[0].length,
				numberOfChannels: channels.length,
				timestamp: (context.currentTime * 1e6) | 0,
				data: joined,
				transfer: [joined.buffer],
			});

			encoder.encode(frame);
			frame.close();
		});
	}

	close() {
		this.#signals.close();
	}
}

function worklet() {
	// @ts-expect-error Would need a separate file/tsconfig to get this to work.
	registerProcessor(
		"capture",
		// @ts-expect-error Would need a separate tsconfig to get this to work.
		class Processor extends AudioWorkletProcessor {
			process(input: Float32Array[][]) {
				// @ts-expect-error Would need a separate tsconfig to get this to work.
				this.port.postMessage(input[0]);
				return true;
			}
		},
	);
}
