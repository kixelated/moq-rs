import * as Moq from "..";

export class MoqMeetElement extends HTMLElement {
	#meet?: Moq.Meet;
	#url: string | null = null;
	#canvas: HTMLCanvasElement | null = null;
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
<slot name="canvas"></slot>

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
		this.#canvas = this.querySelector("canvas");
		if (!this.#canvas) {
			throw new Error("Canvas element is required");
		}

		this.#camera = this.#shadow.querySelector("#camera");
		this.#camera?.addEventListener("click", async () => {
			const media = await navigator.mediaDevices.getUserMedia({ video: true });
		});

		for (const name of MoqMeetElement.observedAttributes) {
			const value = this.getAttribute(name);
			if (value !== null) {
				this.attributeChangedCallback(name, null, this.getAttribute(name));
			}
		}
	}

	disconnectedCallback() {
		this.#canvas?.close();
	}

	attributeChangedCallback(name: string, old: string | null, value: string | null) {
		if (old === value) {
			return;
		}

		switch (name) {
			case "url":
				this.#url = value;

				this.#meet?.close();

				if (this.#url) {
					this.#meet = new Moq.Meet(this.#url);
				}

				break;
		}
	}
}

customElements.define("moq-meet", MoqMeetElement);

declare global {
	interface HTMLElementTagNameMap {
		"moq-meet": MoqMeetElement;
	}
}
