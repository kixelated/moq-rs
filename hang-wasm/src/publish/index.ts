import { Bridge } from "../bridge";

export class Publish {
	#bridge = new Bridge();

	#url: URL | null = null;
	#device: "camera" | "screen" | null = null;
	#video = true;
	#audio = true;
	#preview: HTMLVideoElement | null = null;

	get url(): URL | null {
		return this.#url;
	}

	set url(url: URL | null) {
		this.#url = url;
		this.#bridge.postMessage({ Publish: { Connect: url?.toString() ?? null } });
	}

	get device(): "camera" | "screen" | null {
		return this.#device;
	}

	get video(): boolean {
		return this.#video;
	}

	get audio(): boolean {
		return this.#audio;
	}

	set device(device: "camera" | "screen" | null) {
		this.#device = device;
	}

	set video(video: boolean) {
		this.#video = video;
	}

	set audio(audio: boolean) {
		this.#audio = audio;
	}

	get preview(): HTMLVideoElement | null {
		return this.#preview;
	}

	set preview(preview: HTMLVideoElement | null) {
		this.#preview = preview;
	}
}

// A custom element making it easier to insert into the DOM.
export class PublishElement extends HTMLElement {
	static observedAttributes = ["url", "device", "no-video", "no-audio"];

	// Expose the library instance for easy access.
	readonly lib = new Publish();

	constructor() {
		super();

		// Attach a <video> element to the root used for previewing the video.
		const video = document.createElement("video");
		this.lib.preview = video;

		const slot = document.createElement("slot");
		slot.addEventListener("slotchange", () => {
			for (const el of slot.assignedElements({ flatten: true })) {
				if (el instanceof HTMLVideoElement) {
					this.lib.preview = el;
					return;
				}
			}

			this.lib.preview = null;
		});
		slot.appendChild(video);

		this.attachShadow({ mode: "open" }).appendChild(slot);
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.lib.url = newValue ? new URL(newValue) : null;
		} else if (name === "device") {
			this.lib.device = newValue as "camera" | "screen" | null;
		} else if (name === "no-video") {
			this.lib.video = !newValue;
		} else if (name === "no-audio") {
			this.lib.audio = !newValue;
		}
	}
}

customElements.define("hang-publish", PublishElement);

declare global {
	interface HTMLElementTagNameMap {
		"hang-publish": PublishElement;
	}
}
