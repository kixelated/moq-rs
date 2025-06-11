import * as Moq from "@kixelated/moq";
import { Signal, Signals, cleanup, signal } from "@kixelated/signals";
import * as Catalog from "../catalog";
import { Connection } from "../connection";
import { Audio, AudioConstraints, AudioTrack } from "./audio";
import { Chat, ChatProps } from "./chat";
import { Location, LocationProps } from "./location";
import { Video, VideoConstraints, VideoTrack } from "./video";

export type Device = "screen" | "camera";

export type BroadcastProps = {
	enabled?: boolean;
	path?: string;
	audio?: AudioConstraints | boolean;
	video?: VideoConstraints | boolean;
	location?: LocationProps;
	user?: Catalog.User;
	device?: Device;
	chat?: ChatProps;

	// You can disable reloading if you want to save a round trip when you know the broadcast is already live.
	reload?: boolean;
};

export class Broadcast {
	connection: Connection;
	enabled: Signal<boolean>;
	path: Signal<string>;

	audio: Audio;
	video: Video;
	location: Location;
	user: Signal<Catalog.User | undefined>;
	chat: Chat;

	//catalog: Memo<Catalog.Root>;
	device: Signal<Device | undefined>;

	#broadcast = new Moq.BroadcastProducer();
	#catalog = new Moq.TrackProducer("catalog.json", 0);
	signals = new Signals();

	#published = signal(false);
	readonly published = this.#published.readonly();

	constructor(connection: Connection, props?: BroadcastProps) {
		this.connection = connection;
		this.enabled = signal(props?.enabled ?? false);
		this.path = signal(props?.path ?? "");

		this.audio = new Audio(this.#broadcast, { constraints: props?.audio });
		this.video = new Video(this.#broadcast, { constraints: props?.video });
		this.location = new Location(this.#broadcast, props?.location);
		this.chat = new Chat(this.#broadcast, props?.chat);
		this.user = signal(props?.user);

		this.device = signal(props?.device);

		this.#broadcast.insertTrack(this.#catalog.consume());

		this.signals.effect(() => {
			if (!this.enabled.get()) return;

			const connection = this.connection.established.get();
			if (!connection) return;

			const path = this.path.get();
			if (path === undefined) return;

			// Publish the broadcast to the connection.
			const consume = this.#broadcast.consume();

			// Unpublish the broadcast by closing the consumer but not the publisher.
			cleanup(() => consume.close());
			connection.publish(path, consume);

			this.#published.set(true);
			cleanup(() => this.#published.set(false));
		});

		// These are separate effects because the camera audio/video constraints can be independent.
		// The screen constraints are needed at the same time.
		this.signals.effect(() => this.#runCameraAudio());
		this.signals.effect(() => this.#runCameraVideo());
		this.signals.effect(() => this.#runScreen());

		this.signals.effect(() => this.#runCatalog());
	}

	#runCameraAudio() {
		const device = this.device.get();
		if (device !== "camera") return;

		const audio = this.audio.constraints.get();
		if (!audio) return;

		const media = navigator.mediaDevices.getUserMedia({ audio });

		media
			.then((media) => {
				const track = media.getAudioTracks().at(0);
				this.audio.media.set(track as AudioTrack | undefined);
			})
			.catch((err) => {
				console.error("failed to get media", err);
			});

		return () => {
			this.audio.media.set((prev) => {
				prev?.stop();
				return undefined;
			});
		};
	}

	#runCameraVideo() {
		const device = this.device.get();
		if (device !== "camera") return;

		const video = this.video.constraints.get();
		if (!video) return;

		const media = navigator.mediaDevices.getUserMedia({ video });

		media
			.then((media) => {
				const track = media.getVideoTracks().at(0);
				this.video.media.set(track as VideoTrack | undefined);
			})
			.catch((err) => {
				console.error("failed to get media", err);
			});

		return () => {
			this.video.media.set((prev) => {
				prev?.stop();
				return undefined;
			});
		};
	}

	#runScreen() {
		const device = this.device.get();
		if (device !== "screen") return;

		const audio = this.audio.constraints.get();
		const video = this.video.constraints.get();
		if (!audio && !video) return;

		// TODO Expose these to the application.
		// @ts-expect-error Chrome only
		let controller: CaptureController | undefined;
		// @ts-expect-error Chrome only
		if (typeof self.CaptureController !== "undefined") {
			// @ts-expect-error Chrome only
			controller = new CaptureController();
			controller.setFocusBehavior("no-focus-change");
		}

		const media = navigator.mediaDevices.getDisplayMedia({
			video,
			audio,
			// @ts-expect-error Chrome only
			controller,
			preferCurrentTab: false,
			selfBrowserSurface: "exclude",
			surfaceSwitching: "include",
			// TODO We should try to get system audio, but need to be careful about feedback.
			// systemAudio: "exclude",
		});

		media
			.then((media) => {
				const video = media.getVideoTracks().at(0) as VideoTrack | undefined;
				const audio = media.getAudioTracks().at(0) as AudioTrack | undefined;
				this.video.media.set(video);
				this.audio.media.set(audio);
			})
			.catch((err) => {
				console.error("failed to get media", err);
			});

		return () => {
			this.video.media.set((prev) => {
				prev?.stop();
				return undefined;
			});

			this.audio.media.set((prev) => {
				prev?.stop();
				return undefined;
			});
		};
	}

	#runCatalog() {
		if (!this.enabled.get()) return;

		// Create the new catalog.
		const audio = this.audio.catalog.get();
		const video = this.video.catalog.get();

		const catalog: Catalog.Root = {
			video: video ? [video] : [],
			audio: audio ? [audio] : [],
			location: this.location.catalog.get(),
			user: this.user.get(),
			chat: this.chat.catalog.get(),
		};

		const encoded = Catalog.encode(catalog);

		// Encode the catalog.
		const catalogGroup = this.#catalog.appendGroup();
		catalogGroup.writeFrame(encoded);
		catalogGroup.close();

		console.debug("published catalog", this.path.peek(), catalog);
	}

	close() {
		this.signals.close();
		this.audio.close();
		this.video.close();
		this.location.close();
		this.chat.close();
	}
}
