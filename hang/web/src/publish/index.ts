import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import * as Audio from "./audio";
import * as Video from "./video";

import * as Hex from "../util/hex";
import { isAudioTrackSettings, isVideoTrackSettings } from "../util/settings";
import { Abortable, Context } from "../util/context";

export type Device = "screen" | "camera" | "";

export class Publish {
	#url?: string;
	#device?: Device;
	#render?: HTMLVideoElement;

	#connection?: Abortable<Moq.Connection>;
	#media?: Abortable<MediaStream>;
	#running?: Abortable<void>;

	get url(): string | undefined {
		return this.#url;
	}

	set url(url: string | undefined) {
		this.#url = url;
		this.#connection?.abort();

		if (url) {
			this.#connection = new Abortable((context) => this.#connect(context, url));
		} else {
			this.#connection = undefined;
		}
	}

	async #connect(context: Context, url: string): Promise<Moq.Connection> {
		// TODO support abort
		const connection = await Moq.Connection.connect(url);

		context.done.finally(() => connection.close());
		if (!context.aborted) {
			this.#start(connection, this.#media?.current);
		}

		return connection;
	}

	get device(): Device | undefined {
		return this.#device;
	}

	set device(device: Device) {
		this.#device = device;

		if (this.#device === "camera") {
			this.#media = new Abortable(async (context) => {
				const stream = await navigator.mediaDevices.getUserMedia({ video: true });
				context.done.finally(() => {
					for (const track of stream.getTracks()) {
						track.stop();
					}
				});

				if (!context.aborted) {
					this.#start(this.#connection?.current, stream);
				}

				return stream;
			});
		} else if (this.#device === "screen") {
			this.#media = new Abortable(async (context) => {
				const stream = await navigator.mediaDevices.getDisplayMedia({ video: true });
				context.done.finally(() => {
					for (const track of stream.getTracks()) {
						track.stop();
					}
				});

				if (!context.aborted) {
					this.#start(this.#connection?.current, stream);
				}

				return stream;
			});
		} else {
			this.#media?.abort();
			this.#media = undefined;
			return;
		}
	}

	get render(): HTMLVideoElement | undefined {
		return this.#render;
	}

	set render(render: HTMLVideoElement) {
		this.#render = render;
	}

	#start(connection: Moq.Connection | undefined, media: MediaStream | undefined) {
		this.#running?.abort();

		if (!connection || !media) {
			return;
		}

		this.#running = new Abortable((context) => this.#run(context, connection, media));
	}

	async #run(context: Context, connection: Moq.Connection, media: MediaStream) {
		const broadcast = new Catalog.Broadcast();

		// Remove the leading slash
		const path = connection.url.pathname.slice(1);

		for (const track of media.getTracks()) {
			const settings = track.getSettings();

			const info: Catalog.Track = {
				name: track.id, // TODO way too verbose
				priority: track.kind === "video" ? 1 : 2,
			};

			const trackPath = `${path}/${info.name}`;
			const trackProducer = new Moq.Track(trackPath, info.priority);
			context.done.finally(() => trackProducer.close());

			if (isVideoTrackSettings(settings)) {
				const videoTrack = track as MediaStreamVideoTrack;
				const config: Video.EncoderConfig = {
					codec: "vp8", // TODO better default
					height: settings.height,
					width: settings.width,
					framerate: settings.frameRate,
					latencyMode: "realtime",
				};

				const encoder = new Video.Encoder(config, trackProducer);
				encoder
					.run(context, videoTrack)
					.catch(console.error)
					.finally(() => trackProducer.close());

				const decoder = await encoder.decoder();
				const description = decoder.description ? new Uint8Array(decoder.description as ArrayBuffer) : undefined;

				const video: Catalog.Video = {
					track: info,
					codec: decoder.codec,
					description: description ? Hex.encode(description) : undefined,
					resolution: { width: settings.width, height: settings.height },
				};

				broadcast.video.push(video);
			} else if (isAudioTrackSettings(settings)) {
				const audioTrack = track as MediaStreamAudioTrack;
				const config: Audio.EncoderConfig = {
					codec: "Opus", // TODO better default
					sampleRate: settings.sampleRate,
					numberOfChannels: settings.channelCount,
				};

				const encoder = new Audio.Encoder(config, trackProducer);
				encoder
					.run(context, audioTrack)
					.catch(console.error)
					.finally(() => trackProducer.close());

				const audio: Catalog.Audio = {
					track: info,
					codec: config.codec,
					sample_rate: config.sampleRate,
					channel_count: config.numberOfChannels,
				};

				broadcast.audio.push(audio);
			} else {
				throw new Error(`unknown track type: ${track.kind}`);
			}

			connection.publish(trackProducer.reader());
		}

		const catalogPath = `${path}/catalog.json`;
		const catalogProducer = new Moq.Track(catalogPath, 0);
		context.done.finally(() => catalogProducer.close());

		const encoder = new TextEncoder();
		const encoded = encoder.encode(broadcast.encode());
		catalogProducer.appendGroup().writeFrames(encoded);

		connection.publish(catalogProducer.reader());
	}

	close() {
		this.#connection?.abort();
		this.#media?.abort();
		this.#running?.abort();

		this.#connection = undefined;
		this.#media = undefined;
		this.#running = undefined;
	}
}
