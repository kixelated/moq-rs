import * as Moq from "@kixelated/moq";
import { Signal, Signals, signal } from "@kixelated/signals";
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
	readonly media: Signal<AudioTrack | undefined>;
	readonly constraints: Signal<AudioConstraints | boolean | undefined>;

	#catalog = signal<Catalog.Audio | undefined>(undefined);
	readonly catalog = this.#catalog.readonly();

	#track = signal<Moq.TrackProducer | undefined>(undefined);
	readonly track = this.#track.readonly();

	#group?: Moq.GroupProducer;
	#groupTimestamp = 0;

	#id = 0;
	#signals = new Signals();

	constructor(props?: AudioProps) {
		this.media = signal(props?.media);
		this.constraints = signal(props?.constraints);

		this.#signals.effect(() => this.#runCatalog());
		this.#signals.effect(() => this.#runEncoder());
	}

	#runCatalog() {
		const media = this.media.get();
		if (!media) return;

		const track = new Moq.TrackProducer(`audio-${this.#id++}`, 1);
		this.#track.set(track);

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

		return () => {
			track.close();

			this.#catalog.set(undefined);
			this.#track.set(undefined);
		};
	}

	#runEncoder() {
		const media = this.media.get();
		if (!media) return;

		const track = this.#track.get();
		if (!track) return;

		const catalog = this.#catalog.get();
		if (!catalog) return;

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

		const config = catalog.config;

		encoder.configure({
			codec: config.codec,
			numberOfChannels: config.numberOfChannels,
			sampleRate: config.sampleRate,
			bitrate: config.bitrate,
		});

		const processor = AudioTrackProcessor(media);
		const reader = processor.getReader();

		(async () => {
			for (;;) {
				const { value: frame } = await reader.read();
				if (!frame) {
					break;
				}

				encoder.encode(frame);
				frame.close();
			}

			try {
				this.#group?.close();
				this.#group = undefined;

				encoder.close();
			} catch {}
		})();

		return () => {
			reader.cancel();
		};
	}

	close() {
		this.#signals.close();
	}
}

// Firefox doesn't support MediaStreamTrackProcessor so we need to use a polyfill.
// Based on: https://jan-ivar.github.io/polyfills/mediastreamtrackprocessor.js
// Thanks Jan-Ivar
function AudioTrackProcessor(track: AudioTrack): ReadableStream<AudioData> {
	// @ts-expect-error Chrome only for now
	if (self.MediaStreamTrackProcessor) {
		// @ts-expect-error Chrome only for now
		return new self.MediaStreamTrackProcessor({ track }).readable;
	}

	const settings = track.getSettings();
	if (!settings) {
		throw new Error("track has no settings");
	}

	let ac: AudioContext;
	let node: AudioWorkletNode;
	const arrays: Float32Array[][] = [];

	return new ReadableStream({
		async start() {
			ac = new AudioContext({
				// If undefined (Firefox), defaults to the system sample rate.
				sampleRate: settings.sampleRate,
			});

			function worklet() {
				registerProcessor(
					"mstp-shim",
					class Processor extends AudioWorkletProcessor {
						process(input: Float32Array[][]) {
							this.port.postMessage(input[0]);
							return true;
						}
					},
				);
			}
			await ac.audioWorklet.addModule(`data:text/javascript,(${worklet.toString()})()`);
			node = new AudioWorkletNode(ac, "mstp-shim");
			ac.createMediaStreamSource(new MediaStream([track])).connect(node);
			node.port.addEventListener("message", ({ data }: { data: Float32Array[] }) => {
				if (data.length) arrays.push(data);
			});
		},
		async pull(controller) {
			let channels: Float32Array[] | undefined = arrays.shift();

			while (!channels) {
				await new Promise((r) => {
					node.port.onmessage = r;
				});
				channels = arrays.shift();
			}

			const joinedLength = channels.reduce((a, b) => a + b.length, 0);
			const joined = new Float32Array(joinedLength);

			channels.reduce((offset: number, channel: Float32Array): number => {
				joined.set(channel, offset);
				return offset + channel.length;
			}, 0);

			controller.enqueue(
				new AudioData({
					format: "f32-planar",
					sampleRate: ac.sampleRate,
					numberOfFrames: channels[0].length,
					numberOfChannels: channels.length,
					timestamp: (ac.currentTime * 1e6) | 0,
					data: joined,
					transfer: [joined.buffer],
				}),
			);
		},
	});
}
