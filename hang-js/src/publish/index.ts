import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import { Audio } from "./audio";
import { Video } from "./video";

import * as Hex from "../util/hex";
import { VideoTrackSettings, AudioTrackSettings } from "../util/settings";
import { Broadcast } from "./broadcast";

export type Device = "screen" | "camera";

export class Publish {
	#url?: URL;
	#device?: Device;
	#preview?: HTMLVideoElement;

	#broadcast?: Promise<Broadcast>;
	#media?: Promise<MediaStream>;

	#audio?: Audio;
	#audioEnabled = true;
	#video?: Video;
	#videoEnabled = true;

	#catalog = new Moq.Track("catalog", 0);

	get audio() {
		return this.#audioEnabled;
	}

	set audio(enabled: boolean) {
		this.#audioEnabled = enabled;
		this.#init();
	}

	get video() {
		return this.#videoEnabled;
	}

	set video(enabled: boolean) {
		this.#videoEnabled = enabled;
		this.#init();
	}

	get url(): URL | undefined {
		return this.#url;
	}

	set url(url: URL | undefined) {
		this.#url = url;

		// Close the old connection.
		this.#broadcast?.then((broadcast) => broadcast.close()).catch(() => {});

		if (url) {
			this.#broadcast = this.#connect(url);
		} else {
			this.#broadcast = undefined;
		}
	}

	async #connect(url: URL): Promise<Broadcast> {
		const name = url.pathname.slice(1);

		// Connect to the URL without the path
		const base = new URL(url);
		base.pathname = "";

		const connection = await Moq.Connection.connect(base);

		const broadcast = new Broadcast(connection, name);

		broadcast.publish(this.#catalog.reader.tee());

		if (this.#audio) {
			broadcast.publish(this.#audio.track.reader.tee());
		}

		if (this.#video) {
			broadcast.publish(this.#video.track.reader.tee());
		}

		return broadcast;
	}

	get device(): Device | undefined {
		return this.#device;
	}

	set device(device: Device | undefined) {
		this.#device = device;
		this.#init();
	}

	get preview(): HTMLVideoElement | undefined {
		return this.#preview;
	}

	set preview(render: HTMLVideoElement | undefined) {
		this.#preview = render;
	}

	async #init() {
		const existing = this.#media;

		// Register our new media request to create a chain.
		// NOTE: We actually await it below.
		if (this.#device === "camera") {
			this.#media = navigator.mediaDevices.getUserMedia({ video: this.#videoEnabled, audio: this.#audioEnabled });
		} else if (this.#device === "screen") {
			this.#media = navigator.mediaDevices.getDisplayMedia({ video: this.#videoEnabled, audio: this.#audioEnabled });
		} else {
			this.#media = undefined;
		}

		// Cancel the old media request first.
		if (existing) {
			const media = await existing;
			for (const track of media.getTracks()) {
				track.stop();
			}
		}

		if (!this.#media) {
			return;
		}

		// Finish the media request.
		const media = await this.#media;

		// Create the new catalog.
		const catalog = new Catalog.Broadcast();

		const audio = media.getAudioTracks().at(0);
		if (!audio || this.#audio?.media !== audio) {
			this.#audio?.close();
			this.#audio = undefined;
		} else if (audio) {
			this.#audio = new Audio(audio);
		}

		if (this.#audio) {
			catalog.audio.push(this.#audio.catalog);
		}

		const video = media.getVideoTracks().at(0);
		if (!video || this.#video?.media !== video) {
			this.#video?.close();
			this.#video = undefined;
		} else if (video) {
			this.#video = new Video(video);
		}

		if (this.#video) {
			catalog.video.push(this.#video.catalog);
		}

		const encoder = new TextEncoder();
		const encoded = encoder.encode(catalog.encode());
		const catalogGroup = this.#catalog.writer.appendGroup();
		catalogGroup.writeFrame(encoded);
		catalogGroup.close();
	}

	close() {
		this.#media?.then((media) => {
			for (const track of media.getTracks()) {
				track.stop();
			}
		});

		this.#broadcast?.then((broadcast) => broadcast.close());
	}
}

export default Publish;
