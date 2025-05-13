import * as Moq from "@kixelated/moq";

import { Broadcast, Device } from "./broadcast";

export class Publish extends HTMLElement {
	static observedAttributes = ["url", "name", "device", "audio", "video"];

	#name?: string;
	#device?: Device;
	#preview?: HTMLVideoElement;
	#audio = false;
	#video = false;

	#connection?: Moq.ConnectionReload;
	#broadcast?: Broadcast;
	#media?: MediaStream;

	constructor() {
		super();

		const preview = document.createElement("video");
		preview.style.width = "100%";
		preview.style.height = "auto";
		preview.setAttribute("muted", "true");
		preview.setAttribute("autoplay", "true");

		const slot = document.createElement("slot");
		slot.addEventListener("slotchange", () => {
			this.#preview = undefined;

			for (const el of slot.assignedElements({ flatten: true })) {
				if (el instanceof HTMLVideoElement) {
					this.#preview = el;
					break;
				}
			}

			if (this.#preview && this.#media) {
				this.#preview.srcObject = this.#media;
			}
		});

		slot.appendChild(preview);
		this.#preview = preview;

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
			this.url = newValue ? new URL(newValue) : undefined;
		} else if (name === "name") {
			this.name = newValue;
		} else if (name === "device") {
			this.device = newValue as Device;
		} else if (name === "audio") {
			this.audio = newValue !== undefined;
		} else if (name === "video") {
			this.video = newValue !== undefined;
		}
	}

	get url() {
		return this.#connection?.url;
	}

	set url(url: URL | undefined) {
		this.#connection?.close();
		this.#connection = undefined;

		if (!url) {
			return;
		}

		this.#connection = new Moq.ConnectionReload(url);

		this.#connection.on("connecting", () => {
			this.dispatchEvent(new CustomEvent("moq-connection", { detail: "connecting" }));
		});

		this.#connection.on("connected", () => {
			this.dispatchEvent(new CustomEvent("moq-connection", { detail: "connected" }));
		});

		this.#connection.on("disconnected", () => {
			this.dispatchEvent(new CustomEvent("moq-connection", { detail: "disconnected" }));
		});

		this.#run();
	}

	get name() {
		return this.#name;
	}

	set name(name: string | undefined) {
		this.#name = name;
		this.#run();
	}

	get device() {
		return this.#device;
	}

	set device(device: Device | undefined) {
		this.#device = device;

		if (this.#broadcast) {
			this.#broadcast.device = device;
		}
	}

	get audio() {
		return this.#audio;
	}

	set audio(audio: boolean) {
		this.#audio = audio;

		if (this.#broadcast) {
			this.#broadcast.audio = audio;
		}
	}

	get video() {
		return this.#video;
	}

	set video(video: boolean) {
		this.#video = video;

		if (this.#broadcast) {
			this.#broadcast.video = video;
		}
	}

	async #run() {
		this.#stop();

		if (!this.#connection || !this.#name) {
			return;
		}

		this.#broadcast = new Broadcast(this.#connection, this.#name, {
			device: this.#device,
			audio: this.#audio,
			video: this.#video,
			onMedia: (media) => {
				this.#media = media;

				if (this.#preview) {
					this.#preview.srcObject = media ?? null;
				}
			},
		});
	}

	#stop() {
		this.#broadcast?.close();
		this.#broadcast = undefined;
	}
}

customElements.define("hang-publish", Publish);

declare global {
	interface HTMLElementTagNameMap {
		"hang-publish": Publish;
	}
}

declare global {
	interface HTMLElementEventMap {
		"moq-connection": CustomEvent<keyof Moq.ConnectionStatus>;
	}
}
