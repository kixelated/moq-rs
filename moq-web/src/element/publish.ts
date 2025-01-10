import * as Moq from "..";

export class MoqPublishElement extends HTMLElement {
	#publish?: Moq.Publish;
	#url?: string;
	#media?: MediaStream;

	#shadow: ShadowRoot;
	#preview?: HTMLVideoElement;
	#camera?: HTMLButtonElement;
	#screen?: HTMLButtonElement;

	static get observedAttributes() {
		return ["url"];
	}

	constructor() {
		super();

		const shadow = this.attachShadow({ mode: "open" });
		shadow.innerHTML = `
<style>
	:host {
		display: block;
		position: relative;
	}

	::slotted(video) {
		display: block;
		max-width: 100%;
		height: auto;
	}

	#controls {
		margin-top: 10px;
	}
</style>
<slot name="preview"></slot>

<div id="controls">
	<button id="camera">Camera</button>
	<button id="screen">Screen</button>
</div>
`;
		this.#shadow = shadow;
	}

	setAttr(name: string, oldValue?: string, newValue?: string) {
		this.attributeChangedCallback(name, oldValue, newValue);
	}

	connectedCallback() {
		this.#preview = this.querySelector("video") ?? undefined;

		this.#camera = this.#shadow.querySelector("#camera") ?? undefined;
		this.#camera?.addEventListener("click", async () => {
			this.#media = await navigator.mediaDevices.getUserMedia({
				video: true,
			});

			for (const track of this.#media.getTracks()) {
				console.log(track.getSettings(), track.getCapabilities(), track.getConstraints());
			}

			this.#publish?.capture(this.#media);

			if (this.#preview) {
				this.#preview.srcObject = this.#media;
			}
		});

		this.#screen = this.#shadow.querySelector("#screen") ?? undefined;
		this.#screen?.addEventListener("click", async () => {
			this.#media = await navigator.mediaDevices.getDisplayMedia({ video: true });
			this.#publish?.capture(this.#media);

			if (this.#preview) {
				this.#preview.srcObject = this.#media;
			}
		});

		for (const name of MoqPublishElement.observedAttributes) {
			const value = this.getAttribute(name) ?? undefined;
			if (value !== undefined) {
				this.attributeChangedCallback(name, undefined, value);
			}
		}
	}

	disconnectedCallback() {
		this.#publish?.close();
	}

	attributeChangedCallback(name: string, old?: string, value?: string) {
		if (old === value) {
			return;
		}

		switch (name) {
			case "url":
				this.#url = value;

				this.#publish?.close();

				if (this.#url) {
					this.#publish = new Moq.Publish(this.#url);
					this.#publish.capture(this.#media);
				}

				break;
		}
	}
}

customElements.define("moq-publish", MoqPublishElement);

declare global {
	interface HTMLElementTagNameMap {
		"moq-publish": MoqPublishElement;
	}
}
