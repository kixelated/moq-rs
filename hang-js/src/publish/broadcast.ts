import * as Moq from "@kixelated/moq";
import * as Media from "../media";
import { Audio } from "./audio";
import { Video } from "./video";
import { Connection } from "../connection"
import { Signals, Signal, signal, Derived } from "../signals"

export type Device = "screen" | "camera";

export type BroadcastProps = {
	connection: Connection;

	name?: string;
	audio?: boolean;
	video?: boolean;
	device?: Device;

	// You can disable reloading if you want to save a round trip when you know the broadcast is already live.
	reload?: boolean;
};

export class Broadcast {
	connection: Connection;
	name: Signal<string | undefined>;

	#media = signal<MediaStream | undefined>(undefined);
	media = this.#media.readonly();

	audio: Audio;
	video: Video;
	catalog: Derived<Media.Catalog>;
	device: Signal<Device | undefined>;

	#broadcast = signal<Moq.Broadcast | undefined>(undefined);
	#catalog = new Moq.Track("catalog.json", 0);
	#signals = new Signals();

	constructor(props: BroadcastProps) {
		this.connection = props.connection;
		this.name = signal(props.name);
		this.audio = new Audio({ enabled: props.audio});
		this.video = new Video({ enabled: props.video});
		this.device = signal(props.device);

		this.#signals.effect(() => {
			const name = this.name.get();
			if (!name) return;

			const broadcast = new Moq.Broadcast(name);
			broadcast.writer.insertTrack(this.#catalog.reader.clone());

			this.#broadcast.set(broadcast);

			return () => {
				broadcast.close();
				this.#broadcast.set(undefined);
			}
		});

		this.#signals.effect(() => {
			const broadcast = this.#broadcast.get();
			if (!broadcast) return;

			const connection = this.connection.established.get();
			if (!connection) return;

			// Publish the broadcast to the connection.
			connection.publish(broadcast.reader.clone());
		});

		this.#signals.effect(() => {
			const broadcast = this.#broadcast.get();
			if (!broadcast) return;

			const track = this.video.track.get();
			if (!track) return;

			broadcast.writer.insertTrack(track.reader.clone());

			return () => {
				broadcast.writer.removeTrack(track.name);
			}
		});

		this.#signals.effect(() => {
			const broadcast = this.#broadcast.get();
			if (!broadcast) return;

			const track = this.audio.track.get();
			if (!track) return;

			broadcast.writer.insertTrack(track.reader.clone());
			return () => {
				broadcast.writer.removeTrack(track.name);
			}
		});

		this.#signals.effect(() => this.#promptUser());
		this.#signals.effect(() => {
			this.audio.media.set(this.media.get()?.getAudioTracks().at(0));
			this.video.media.set(this.media.get()?.getVideoTracks().at(0));
		});

		this.catalog = this.#signals.derived(() => this.#runCatalog());
	}

	#promptUser() {
		const device = this.device.get();
		if (!device) return;

		const videoEnabled = this.video.enabled.get();
		const audioEnabled = this.audio.enabled.get();
		if (!videoEnabled && !audioEnabled) return;

		let media: Promise<MediaStream>;

		if (device === "camera") {
			const video = videoEnabled
				? {
						//aspectRatio: { ideal: 16 / 9 },
						width: { ideal: 1280, max: 1920 },
						height: { ideal: 720, max: 1080 },
						frameRate: { ideal: 60, max: 60 },
						facingMode: { ideal: "user" },
					}
				: undefined;

			const audio = audioEnabled
				? {
						channelCount: { ideal: 2, max: 2 },
						echoCancellation: { ideal: true },
						autoGainControl: { ideal: true },
						noiseCancellation: { ideal: true },
					}
				: undefined;

			media = navigator.mediaDevices.getUserMedia({ video, audio });
		} else if (device === "screen") {
			const video = videoEnabled
				? {
						frameRate: { ideal: 60, max: 60 },
						preferCurrentTab: false,
						selfBrowserSurface: "exclude",
						surfaceSwitching: "include",
						systemAudio: "include",
						resizeMode: "none",
					}
				: undefined;

			const audio = audioEnabled
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

			media = navigator.mediaDevices.getDisplayMedia({
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

		media.then((media) => {
			this.#media.set(media);
		}).catch((err) => {
			console.error("failed to get media", err);
		});

		return () => {
			this.#media.set(undefined);
		}
	}

	#runCatalog(): Media.Catalog {
		const audio = this.audio.catalog.get();
		const video = this.video.catalog.get();

		// Create the new catalog.
		const catalog = new Media.Catalog();

		// We need to wait for the encoder to fully initialize with a few frames.
		if (audio) {
			catalog.audio.push(audio);
		}

		if (video) {
			catalog.video.push(video);
		}

		const encoder = new TextEncoder();
		const encoded = encoder.encode(catalog.encode());
		const catalogGroup = this.#catalog.writer.appendGroup();
		catalogGroup.writeFrame(encoded);
		catalogGroup.close();

		console.debug("published catalog", catalog);

		return catalog;
	}

	close() {
		this.#signals.close();
		this.audio.close();
		this.video.close();
	}
}
