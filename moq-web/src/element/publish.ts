import * as Moq from "..";

export class MoqPublishElement extends HTMLElement {
	#publish?: Moq.Publish;
	#url: string | null = null;
	#preview: HTMLVideoElement | null = null;
	#shadow: ShadowRoot;

	#camera: HTMLButtonElement | null = null;
	#screen: HTMLButtonElement | null = null;

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

	::slotted(canvas) {
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

	setAttr(name: string, oldValue: string | null, newValue: string | null) {
		this.attributeChangedCallback(name, oldValue, newValue);
	}

	connectedCallback() {
		this.#preview = this.querySelector("video");

		this.#camera = this.#shadow.querySelector("#camera");
		this.#camera?.addEventListener("click", async () => {
			const media = await navigator.mediaDevices.getUserMedia({ video: true });
			if (this.#preview) {
				this.#preview.srcObject = media;
			}
		});

		for (const name of MoqPublishElement.observedAttributes) {
			const value = this.getAttribute(name);
			if (value !== null) {
				this.attributeChangedCallback(name, null, this.getAttribute(name));
			}
		}
	}

	disconnectedCallback() {
		this.#publish?.close();
	}

	attributeChangedCallback(name: string, old: string | null, value: string | null) {
		if (old === value) {
			return;
		}

		switch (name) {
			case "url":
				this.#url = value;

				this.#publish?.close();

				if (this.#url) {
					this.#publish = new Moq.Publish(this.#url);
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
