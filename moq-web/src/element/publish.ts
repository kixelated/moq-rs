import * as Moq from "..";

export class MoqPublishElement extends HTMLElement {
	#publish: Moq.Publish;

	static get observedAttributes() {
		return ["url"];
	}

	constructor() {
		super();

		this.#publish = new Moq.Publish();

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
	}

	setAttr(name: string, oldValue?: string, newValue?: string) {
		this.attributeChangedCallback(name, oldValue, newValue);
	}

	connectedCallback() {
		const preview = this.querySelector("video[slot=preview], [slot=preview] video") ?? undefined;
		this.#publish.preview = preview as HTMLVideoElement | undefined;

		this.querySelector("#camera")?.addEventListener("click", async () => {
			this.#publish.media = await navigator.mediaDevices.getUserMedia({
				video: true,
			});
		});

		this.querySelector("#screen")?.addEventListener("click", async () => {
			this.#publish.media = await navigator.mediaDevices.getDisplayMedia({ video: true });
		});

		for (const name of MoqPublishElement.observedAttributes) {
			const value = this.getAttribute(name) ?? undefined;
			if (value !== undefined) {
				this.attributeChangedCallback(name, undefined, value);
			}
		}
	}

	disconnectedCallback() {
		this.#publish.free();
	}

	attributeChangedCallback(name: string, old?: string, value?: string) {
		if (old === value) {
			return;
		}

		switch (name) {
			case "url":
				this.#publish.url = value ?? undefined;
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
