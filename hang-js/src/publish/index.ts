import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import * as Audio from "./audio";
import * as Video from "./video";

import { Context, Task } from "../util/context";
import * as Hex from "../util/hex";
import { isAudioTrackSettings, isVideoTrackSettings } from "../util/settings";

export type Device = "screen" | "camera" | undefined;

export class Publish {
	#url?: URL;
	#device?: Device;
	#render?: HTMLVideoElement;

	#connecting = new Task(this.#connect.bind(this));
	#media = new Task(this.#request.bind(this));
	#running = new Task(this.#run.bind(this));

	get url(): URL | undefined {
		return this.#url;
	}

	set url(url: URL | undefined) {
		this.#url = url;

		if (url) {
			this.#connecting.start(url);
		} else {
			this.#connecting.abort();
		}
	}

	async #connect(context: Context, url: URL): Promise<Moq.Connection> {
		const connect = Moq.Connection.connect(url);

		// Make a promise to clean up a (successful) connection on abort.
		context.all(connect).then((connection) => connection.close());

		// Wait for the connection to be established, or undefined if aborted.
		const connection = await context.race(connect);
		await this.#running.start(connection, this.#media.latest?.result);

		// Return the connection value, caching it.
		return connection;
	}

	get device(): Device {
		return this.#device;
	}

	set device(device: Device) {
		this.#device = device;

		if (device) {
			this.#media.start(device);
		} else {
			this.#media.abort();
		}
	}

	async #request(context: Context, device: Device): Promise<MediaStream | undefined> {
		let stream: MediaStream | undefined;

		if (device === "camera") {
			stream = await context.race(navigator.mediaDevices.getUserMedia({ video: true }));
		} else if (device === "screen") {
			stream = await context.race(navigator.mediaDevices.getDisplayMedia({ video: true }));
		}

		await this.#running.start(this.#connecting.latest?.result, stream);

		return stream;
	}

	get render(): HTMLVideoElement | undefined {
		return this.#render;
	}

	set render(render: HTMLVideoElement | undefined) {
		this.#render = render;
	}

	async #run(context: Context, connection?: Moq.Connection, media?: MediaStream) {
		if (!connection || !media) return;

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
			const trackProducer = new Moq.TrackWriter(trackPath, info.priority);
			context.done.then(() => trackProducer.close());

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
		const catalogProducer = new Moq.TrackWriter(catalogPath, 0);
		context.done.then(() => catalogProducer.close());

		const encoder = new TextEncoder();
		const encoded = encoder.encode(broadcast.encode());
		catalogProducer.appendGroup().writeFrames(encoded);

		connection.publish(catalogProducer.reader());
	}

	close() {
		this.#connecting?.abort();
		this.#media?.abort();
		this.#running?.abort();
	}
}

export default Publish;
