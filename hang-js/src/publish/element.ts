import { Broadcast, Device } from "./broadcast";
import { Connection } from "../connection"
import { signal, Signals } from "../signals"

export class Publish extends HTMLElement {
	static observedAttributes = ["url", "name", "device", "audio", "video"];

	connection = new Connection();
	broadcast = new Broadcast({ connection: this.connection });
	preview = signal<HTMLVideoElement | undefined>(undefined);

	#signals = new Signals();

	constructor() {
		super();

		const preview = document.createElement("video");
		preview.style.width = "100%";
		preview.style.height = "auto";
		preview.setAttribute("muted", "true");
		preview.setAttribute("autoplay", "true");

		const slot = document.createElement("slot");
		slot.addEventListener("slotchange", () => {
			for (const el of slot.assignedElements({ flatten: true })) {
				if (el instanceof HTMLVideoElement) {
					this.preview.set(el);
					return;
				}
			}

			this.preview.set(undefined);
		});

		slot.appendChild(preview);
		this.preview.set(preview);

		const style = document.createElement("style");
		style.textContent = `
			:host {
				display: flex;
				align-items: center;
				justify-content: center;
			}
		`;

		this.attachShadow({ mode: "open" }).append(style, slot);

		this.#signals.effect(() => {
			const media = this.broadcast.video.media.get();
			const preview = this.preview.get();
			if (!preview || !media) return;

			preview.srcObject = new MediaStream([ media ]) ?? null;
			return () => {
				preview.srcObject = null;
			}
		});
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.connection.url.set(newValue ? new URL(newValue) : undefined);
		} else if (name === "name") {
			this.broadcast.path.set(newValue);
		} else if (name === "device") {
			this.broadcast.device.set(newValue as Device);
		} else if (name === "audio") {
			this.broadcast.audio.constraints.set(newValue !== undefined);
		} else if (name === "video") {
			this.broadcast.video.constraints.set(newValue !== undefined);
		}
	}
}

customElements.define("hang-publish", Publish);

declare global {
	interface HTMLElementTagNameMap {
		"hang-publish": Publish;
	}
}
