import { Frame } from "../media/frame";

import * as Moq from "@kixelated/moq";
import * as Media from "../media";
import { AudioTrackSettings } from "../util/settings";
import { Signal, Signals, signal } from "../signals"

// Create a group every half a second
const GOP_DURATION = 0.5;

export type AudioTrackConstraints = Omit<MediaTrackConstraints,
  | "aspectRatio"
  | "backgroundBlur"
  | "displaySurface"
  | "facingMode"
  | "frameRate"
  | "height"
  | "width"
>;

export type AudioProps = {
	media?: MediaStreamAudioTrack;
	constraints?: AudioTrackConstraints | boolean;
};

export class Audio {
	readonly media: Signal<MediaStreamAudioTrack | undefined>;
	readonly constraints: Signal<AudioTrackConstraints | boolean | undefined>;

	#catalog = signal<Media.Audio | undefined>(undefined);
	readonly catalog = this.#catalog.readonly();

	#track = signal<Moq.Track | undefined>(undefined);
	readonly track = this.#track.readonly();

	#group?: Moq.GroupWriter;
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

		const track = new Moq.Track(`audio-${this.#id++}`, 1);
		this.#track.set(track);

		const settings = media.getSettings() as AudioTrackSettings;

		const catalog = {
			track: {
				name: track.name,
				priority: track.priority,
			},
			codec: "Opus",
			sample_rate: settings.sampleRate,
			channel_count: settings.channelCount,
			bitrate: 64_000, // TODO support higher bitrates
		}

		this.#catalog.set(catalog);

		return () => {
			track.close();

			this.#catalog.set(undefined);
			this.#track.set(undefined);
		}
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
					this.#group = track.writer.appendGroup();
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

				track.writer.abort(err);
			}
		});

		encoder.configure({
			codec: catalog.codec,
			numberOfChannels: catalog.channel_count,
			sampleRate: catalog.sample_rate,
			bitrate: catalog.bitrate,
		});

		const input = new MediaStreamTrackProcessor({ track: media });
		const reader = input.readable.getReader();

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
