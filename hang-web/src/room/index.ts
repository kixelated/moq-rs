import { Bridge } from "../bridge";
import { Publish } from "../publish";
import { Watch } from "../watch";

export class Room {
	#bridge = new Bridge();
	#url: URL | null = null;

	publish = new Publish(this.#bridge);
	watch = new Watch(this.#bridge);

	get url(): URL | null {
		return this.#url;
	}

	set url(url: URL | null) {
		this.#bridge.postMessage({ Connect: { url: url?.toString() ?? null } });
		this.#url = url;
	}
}

// A custom element making it easier to insert into the DOM.
export class RoomElement extends HTMLElement {
	static observedAttributes = ["url"];

	#hang = new Room();

	constructor() {
		super();

		const canvas = document.createElement("canvas");

		const slot = document.createElement("slot");
		slot.addEventListener("slotchange", () => this.#updateCanvas(slot));
		slot.appendChild(canvas);

		this.attachShadow({ mode: "open" }).appendChild(slot);
	}

	#updateCanvas(slot: HTMLSlotElement) {
		for (const el of slot.assignedElements({ flatten: true })) {
			if (el instanceof HTMLCanvasElement) {
				this.#hang.watch.canvas = el;
				return;
			}
		}

		this.#hang.watch.canvas = null;
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.#hang.url = newValue ? new URL(newValue) : null;
		}
	}
}

customElements.define("hang-room", RoomElement);

declare global {
	interface HTMLElementTagNameMap {
		"hang-room": RoomElement;
	}
}
