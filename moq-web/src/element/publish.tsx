import * as Moq from "..";
import { attribute } from "./component";

import { jsx } from "./jsx";

const observedAttributes = ["url", "media"] as const;
type ObservedAttribute = (typeof observedAttributes)[number];

export class MoqPublishElement extends HTMLElement {
	#publish: Moq.Publish | null;
	#preview: HTMLVideoElement;
	#media: MediaStream | null = null;

	@attribute
	accessor url = "";

	@attribute
	accessor media: "camera" | "screen" | "" = "";

	static get observedAttributes() {
		return observedAttributes;
	}

	constructor() {
		super();

		this.#publish = new Moq.Publish();
		this.#preview = (<video css={{ display: "block", maxWidth: "100%", height: "auto" }} />) as HTMLVideoElement;

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(this.#preview);
	}

	async #shareCamera() {
		// TODO configure the constraints
		this.#media = await navigator.mediaDevices.getUserMedia({
			video: true,
		});

		if (!this.#publish) {
			// Removed from DOM during await.
			return;
		}

		this.#publish.media = this.#media;
		this.#preview.srcObject = this.#media;
	}

	async #shareScreen() {
		this.#media = await navigator.mediaDevices.getDisplayMedia({ video: true });

		if (!this.#publish) {
			// Removed from DOM during await.
			return;
		}

		this.#publish.media = this.#media;
		this.#preview.srcObject = this.#media;
	}

	connectedCallback() {
		if (!this.#publish) {
			this.#publish = new Moq.Publish();
		}

		for (const name of MoqPublishElement.observedAttributes) {
			const value = this.getAttribute(name) ?? undefined;
			if (value !== undefined) {
				this.attributeChangedCallback(name, undefined, value);
			}
		}
	}

	disconnectedCallback() {
		this.#publish?.free();
		this.#publish = null;
	}

	attributeChangedCallback(name: ObservedAttribute, old?: string, value?: string) {
		if (!this.#publish) {
			return;
		}

		if (old === value) {
			return;
		}

		switch (name) {
			case "url":
				this.#publish.url = value ?? undefined;
				break;
			case "media":
				if (value === "camera") {
					this.#shareCamera();
				} else if (value === "screen") {
					this.#shareScreen();
				} else {
					// biome-ignore lint/complexity/noForEach: media may be null
					this.#media?.getTracks().forEach((track) => track.stop());
					this.#media = null;
					this.#publish.media = null;
				}

				break;
			default: {
				// Exhaustiveness check ensures all attributes are handled
				const _exhaustive: never = name;
				throw new Error(`Unhandled attribute: ${_exhaustive}`);
			}
		}
	}
}

customElements.define("moq-publish", MoqPublishElement);

declare global {
	interface HTMLElementTagNameMap {
		"moq-publish": MoqPublishElement;
	}
}
