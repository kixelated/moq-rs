import * as Moq from "..";

export class MoqWatchElement extends HTMLElement {
	#watch?: Moq.Watch;
	#url?: string;

	#shadow: ShadowRoot;
	#canvas?: HTMLCanvasElement;

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
</style>
<slot name="canvas"></slot>
`;
		this.#shadow = shadow;
	}

	setAttr(name: string, oldValue?: string, newValue?: string) {
		this.attributeChangedCallback(name, oldValue, newValue);
	}

	connectedCallback() {
		this.#canvas = this.querySelector("canvas") ?? undefined;

		// Provide a default canvas if none is slotted
		if (!this.#canvas) {
			this.#canvas = document.createElement("canvas");
			this.#canvas.slot = "canvas";
			this.appendChild(this.#canvas);
		}

		this.#watch?.render(this.#canvas);

		if (!this.#canvas) {
			throw new Error("MoqWatchElement requires a canvas element");
		}

		for (const name of MoqWatchElement.observedAttributes) {
			const value = this.getAttribute(name) ?? undefined;
			if (value !== undefined) {
				this.attributeChangedCallback(name, undefined, value);
			}
		}
	}

	disconnectedCallback() {
		this.#watch?.close();
	}

	attributeChangedCallback(name: string, old?: string, value?: string) {
		if (old === value) {
			return;
		}

		switch (name) {
			case "url":
				this.#url = value;

				this.#watch?.close();

				if (this.#url) {
					this.#watch = new Moq.Watch();
				}

				break;
		}
	}
}

customElements.define("moq-watch", MoqWatchElement);

declare global {
	interface HTMLElementTagNameMap {
		"moq-watch": MoqWatchElement;
	}
}
