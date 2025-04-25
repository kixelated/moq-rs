import * as Moq from "@kixelated/moq";
import * as Catalog from "../catalog";
import { Audio } from "./audio";
import { Video } from "./video";

export interface PublishEvents {
	status: PublishStatus;
}

export type Device = "screen" | "camera";
export type PublishStatus = "connecting" | "connected" | "disconnected" | "live";

export class Publish {
	#url?: URL;
	#device?: Device;
	#preview?: HTMLVideoElement;

	#connection?: Promise<Moq.Connection>;
	#broadcast?: Moq.BroadcastWriter;
	#media?: Promise<MediaStream>;

	#audio?: Audio;
	#audioEnabled = true;
	#video?: Video;
	#videoEnabled = true;

	#catalog = new Moq.Track("catalog.json", 0);

	#events = new EventTarget();

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
		this.#connection?.then((connection) => connection.close()).catch(() => {});

		if (url) {
			this.#dispatchEvent("status", "connecting");
			this.#connection = this.#connect(url);
			this.#connection.catch(() => this.#dispatchEvent("status", "disconnected"));
		} else {
			this.#dispatchEvent("status", "disconnected");
			this.#connection = undefined;
		}
	}

	async #connect(url: URL): Promise<Moq.Connection> {
		// TODO separate this from the URL.
		const broadcast = new Moq.Broadcast(url.pathname.slice(1));

		// Connect to the URL without the path
		const base = new URL(url);
		base.pathname = "";

		const connection = await Moq.Connection.connect(base);

		this.#broadcast = broadcast.writer;
		connection.publish(broadcast.reader);

		this.#broadcast.insert(this.#catalog.reader.clone());

		if (this.#audio) {
			this.#broadcast.insert(this.#audio.track.reader.clone());
		}

		if (this.#video) {
			this.#broadcast.insert(this.#video.track.reader.clone());
		}

		if (this.#audio || this.#video) {
			this.#dispatchEvent("status", "live");
		} else {
			this.#dispatchEvent("status", "connected");
		}

		return connection;
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

	set preview(preview: HTMLVideoElement | undefined) {
		if (this.#preview) {
			this.#preview.srcObject = null;
		}

		this.#preview = preview;

		if (preview) {
			this.#media?.then((media) => {
				preview.srcObject = media;
				preview.play();
			});
		}
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
		if (this.#preview) {
			this.#preview.srcObject = media;
			this.#preview.play();
		}

		// Create the new catalog.
		const catalog = new Catalog.Broadcast();

		const audio = media.getAudioTracks().at(0);
		if (this.#audio?.media !== audio) {
			this.#audio?.close();
			this.#audio = undefined;

			if (audio) {
				this.#audio = new Audio(audio);
				this.#broadcast?.insert(this.#audio.track.reader.clone());
			}
		}

		if (this.#audio) {
			catalog.audio.push(this.#audio.catalog);
		}

		const video = media.getVideoTracks().at(0);
		if (this.#video?.media !== video) {
			this.#video?.close();
			this.#video = undefined;

			if (video) {
				this.#video = new Video(video);
				this.#broadcast?.insert(this.#video.track.reader.clone());
			}
		}

		if (this.#video) {
			catalog.video.push(this.#video.catalog);
		}

		if (this.#broadcast) {
			if (this.#audio || this.#video) {
				this.#dispatchEvent("status", "live");
			} else {
				this.#dispatchEvent("status", "connected");
			}
		}

		console.debug("updated catalog", catalog);

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

		this.#connection?.then((connection) => connection.close());
	}

	addEventListener<K extends keyof PublishEvents>(type: K, listener: (event: CustomEvent<PublishEvents[K]>) => void) {
		this.#events.addEventListener(type, listener as EventListener);
	}

	removeEventListener<K extends keyof PublishEvents>(
		type: K,
		listener: (event: CustomEvent<PublishEvents[K]>) => void,
	) {
		this.#events.removeEventListener(type, listener as EventListener);
	}

	#dispatchEvent<K extends keyof PublishEvents>(type: K, detail: PublishEvents[K]) {
		this.#events.dispatchEvent(new CustomEvent(type, { detail }));
	}
}

export class PublishElement extends HTMLElement {
	static observedAttributes = ["url"];

	// Expose the library so we don't have to duplicate everything.
	readonly lib: Publish = new Publish();

	constructor() {
		super();

		// Proxy events from the library to the element.
		this.lib.addEventListener("status", (event) => {
			this.dispatchEvent(new CustomEvent("hang-publish-status", { detail: event.detail }));
		});

		const preview = document.createElement("video");
		preview.style.width = "100%";
		preview.style.height = "auto";
		preview.setAttribute("muted", "true");
		preview.setAttribute("autoplay", "true");

		const slot = document.createElement("slot");
		slot.addEventListener("slotchange", () => {
			for (const el of slot.assignedElements({ flatten: true })) {
				if (el instanceof HTMLVideoElement) {
					this.lib.preview = el;
					return;
				}
			}

			this.lib.preview = undefined;
		});

		slot.appendChild(preview);
		this.lib.preview = preview;

		const style = document.createElement("style");
		style.textContent = `
			:host {
				display: flex;
				align-items: center;
				justify-content: center;
			}
		`;

		this.attachShadow({ mode: "open" }).append(style, slot);
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.lib.url = newValue ? new URL(newValue) : undefined;
		}
	}
}

customElements.define("hang-publish", PublishElement);

declare global {
	interface HTMLElementTagNameMap {
		"hang-publish": PublishElement;
	}
}

declare global {
	interface HTMLElementEventMap {
		"hang-publish-status": CustomEvent<PublishStatus>;
	}
}

export default Publish;
