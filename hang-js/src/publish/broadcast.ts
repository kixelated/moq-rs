import * as Moq from "@kixelated/moq";
import * as Media from "../media";
import { Audio } from "./audio";
import { Video } from "./video";

export type Device = "screen" | "camera";

interface BroadcastOptions {
	audio?: boolean;
	video?: boolean;
	device?: Device;
	onMedia?: (media?: MediaStream) => void;
}

export class Broadcast {
	connection: Moq.ConnectionReload;
	name: string;

	#broadcast?: Moq.BroadcastWriter;

	#device?: Device;

	#media?: Promise<MediaStream>;
	onMedia?: (media?: MediaStream) => void; // A callback to expose the media stream.

	#audio?: Audio;
	#audioEnabled: boolean;

	#video?: Video;
	#videoEnabled: boolean;

	#catalog = new Moq.Track("catalog.json", 0);

	constructor(connection: Moq.ConnectionReload, name: string, options: BroadcastOptions = {}) {
		this.name = name;
		this.connection = connection;
		this.#audioEnabled = options.audio ?? false;
		this.#videoEnabled = options.video ?? false;
		this.#device = options.device;
		this.onMedia = options.onMedia;

		this.#init();
	}

	get audio() {
		return this.#audioEnabled;
	}

	set audio(enabled: boolean) {
		if (this.#audioEnabled === enabled) {
			return;
		}

		this.#audioEnabled = enabled;
		this.#init();
	}

	get video() {
		return this.#videoEnabled;
	}

	set video(enabled: boolean) {
		if (this.#videoEnabled === enabled) {
			return;
		}

		this.#videoEnabled = enabled;
		this.#init();
	}

	get device(): Device | undefined {
		return this.#device;
	}

	set device(device: Device | undefined) {
		if (this.#device === device) {
			return;
		}

		this.#device = device;
		this.#init();
	}

	async #init() {
		const existing = this.#media;

		// Register our new media request to create a chain.
		if (!this.#device || (!this.#videoEnabled && !this.#audioEnabled)) {
			this.#media = undefined;
		} else if (this.#device === "camera") {
			const video = this.#videoEnabled
				? {
						//aspectRatio: { ideal: 16 / 9 },
						width: { ideal: 1280, max: 1920 },
						height: { ideal: 720, max: 1080 },
						frameRate: { ideal: 60, max: 60 },
						facingMode: { ideal: "user" },
					}
				: undefined;

			const audio = this.#audioEnabled
				? {
						channelCount: { ideal: 2, max: 2 },
						echoCancellation: { ideal: true },
						autoGainControl: { ideal: true },
						noiseCancellation: { ideal: true },
					}
				: undefined;

			this.#media = navigator.mediaDevices.getUserMedia({ video, audio });
		} else if (this.#device === "screen") {
			const video = this.#videoEnabled
				? {
						frameRate: { ideal: 60, max: 60 },
						preferCurrentTab: false,
						selfBrowserSurface: "exclude",
						surfaceSwitching: "include",
						systemAudio: "include",
						resizeMode: "none",
					}
				: undefined;

			const audio = this.#audioEnabled
				? {
						channelCount: { ideal: 2, max: 2 },
						echoCancellation: false,
						autoGainControl: false,
						noiseCancellation: false,
					}
				: undefined;

			// @ts-ignore new API
			const controller = new CaptureController();
			controller.setFocusBehavior("no-focus-change");

			this.#media = navigator.mediaDevices.getDisplayMedia({
				video,
				audio,
				// @ts-ignore new API
				controller,
				preferCurrentTab: false,
				selfBrowserSurface: "exclude",
				surfaceSwitching: "include",
				// TODO We should try to get system audio, but need to be careful about feedback.
				// systemAudio: "exclude",
			});
		} else {
			throw new Error("Invalid device");
		}

		// Cancel the old media request first.
		if (existing) {
			const media = await existing;
			for (const track of media.getTracks()) {
				track.stop();
			}
		}

		// Finish the media request.
		const media = await this.#media;

		if (this.onMedia) {
			this.onMedia(media);
		}

		const audio = media?.getAudioTracks().at(0);
		const video = media?.getVideoTracks().at(0);

		if (this.#audio?.media !== audio) {
			this.#audio?.close();
			this.#audio = undefined;

			if (audio) {
				this.#audio = new Audio(audio);
			}
		}

		// Check if the video source has changed.
		if (video?.id !== this.#video?.id) {
			this.#video?.close();
			this.#video = undefined;

			if (video) {
				this.#video = await Video.create(video);
			}
		}

		if (!this.#video && !this.#audio) {
			this.#broadcast?.close();
			this.#broadcast = undefined;
			return;
		}

		if (!this.#broadcast) {
			const broadcast = new Moq.Broadcast(this.name);
			this.#broadcast = broadcast.writer;
			this.#broadcast.insertTrack(this.#catalog.reader.clone());

			// Publish the broadcast to the connection.
			this.connection.on("connected", (connection) => {
				connection.publish(broadcast.reader);
			});
			this.connection.established?.publish(broadcast.reader);
		}

		// Create the new catalog.
		const catalog = new Media.Catalog();

		// We need to wait for the encoder to fully initialize with a few frames.
		if (this.#audio) {
			catalog.audio.push(this.#audio.catalog);
			this.#broadcast.insertTrack(this.#audio.track.reader.clone());
		}

		if (this.#video) {
			catalog.video.push(await this.#video.catalog());
			this.#broadcast.insertTrack(this.#video.track.reader.clone());
		}

		console.debug("published catalog", this.#broadcast.path, catalog);

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

		this.#broadcast?.close();
	}
}
