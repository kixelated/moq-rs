import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import * as Audio from "./audio";
import * as Video from "./video";

import * as Hex from "../util/hex";
import { isAudioTrackSettings, isVideoTrackSettings } from "../util/settings";

export interface BroadcastConfig {
	path: string;
	media: MediaStream;
	id?: number;

	audio?: AudioEncoderConfig;
	video?: VideoEncoderConfig;
}

export interface BroadcastConfigTrack {
	codec: string;
	bitrate: number;
}

export class Broadcast {
	#config: BroadcastConfig;
	#path: string;

	constructor(config: BroadcastConfig) {
		const id = config.id || (new Date().getTime() / 1000).toFixed(0);

		this.#config = config;
		this.#path = config.path.concat(id.toString());
	}

	async publish(connection: Moq.Connection) {
		const broadcast = new Catalog.Broadcast();

		for (const media of this.#config.media.getTracks()) {
			const settings = media.getSettings();

			const info: Catalog.Track = {
				name: media.id, // TODO way too verbose
				priority: media.kind === "video" ? 1 : 2,
			};

			const track = new Moq.Track(this.#path.concat(info.name), info.priority);

			if (isVideoTrackSettings(settings)) {
				if (!this.#config.video) {
					throw new Error("no video configuration provided");
				}

				const encoder = new Video.Encoder(this.#config.video);
				const packer = new Video.Packer(media as MediaStreamVideoTrack, encoder, track);

				// TODO handle error
				packer.run().catch((err) => console.error("failed to run video packer: ", err));

				const decoder = await encoder.decoderConfig();
				const description = decoder.description ? new Uint8Array(decoder.description as ArrayBuffer) : undefined;

				info.bitrate = this.#config.video.bitrate;

				const video: Catalog.Video = {
					track: info,
					codec: decoder.codec,
					description: description ? Hex.encode(description) : undefined,
					resolution: { width: settings.width, height: settings.height },
				};

				broadcast.video.push(video);
			} else if (isAudioTrackSettings(settings)) {
				if (!this.#config.audio) {
					throw new Error("no audio configuration provided");
				}

				const encoder = new Audio.Encoder(this.#config.audio);
				const packer = new Audio.Packer(media as MediaStreamAudioTrack, encoder, track);
				packer.run().catch((err) => console.error("failed to run audio packer: ", err)); // TODO handle error

				const decoder = encoder.decoderConfig();

				info.bitrate = this.#config.audio.bitrate;

				const audio: Catalog.Audio = {
					track: info,
					codec: decoder.codec,
					sample_rate: settings.sampleRate,
					channel_count: settings.channelCount,
				};

				broadcast.audio.push(audio);
			} else {
				throw new Error(`unknown track type: ${media.kind}`);
			}

			connection.publish(track.reader());
		}

		const track = new Moq.Track(this.#path, 0);
		const encoder = new TextEncoder();
		const encoded = encoder.encode(broadcast.encode());
		track.appendGroup().writeFrames(encoded);

		connection.publish(track.reader());
	}

	close() {}
}
