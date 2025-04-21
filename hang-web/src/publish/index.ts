import { Bridge } from "../bridge";
import { Watch } from "../watch";

export class Publish {
	#bridge: Bridge;

	#url: URL | null = null;
	#device: "camera" | "screen" | null = null;
	#video = true;
	#audio = true;

	constructor(bridge: Bridge) {
		this.#bridge = bridge;
	}

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
}

// A custom element making it easier to insert into the DOM.
export class PublishElement extends HTMLElement {
	static observedAttributes = ["url"];

	#bridge = new Bridge();
	#publish = new Publish(this.#bridge);

	constructor() {
		super();

		this.attachShadow({ mode: "open" });
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.#publish.url = newValue ? new URL(newValue) : null;
		}
	}
}

customElements.define("hang-publish", PublishElement);

declare global {
	interface HTMLElementTagNameMap {
		"hang-publish": PublishElement;
	}
}
