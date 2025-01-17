import * as Moq from "..";

import { jsx, jsxFragment } from "./jsx";

export class MoqPublishElement extends HTMLElement {
	#publish: Moq.Publish;
	#preview: HTMLVideoElement;

	static get observedAttributes() {
		return ["url", "preview"];
	}

	constructor() {
		super();

		this.#publish = new Moq.Publish();
		this.#preview = (<video css={{ display: "block", maxWidth: "100%", height: "auto" }} />) as HTMLVideoElement;

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(
			<>
				{this.#preview}

				<div id="controls" css={{ marginTop: "10px" }}>
					<button type="button" onclick={() => this.#shareCamera()}>
						Share Camera
					</button>

					<button type="button" onclick={() => this.#shareScreen()}>
						Share Screen
					</button>
				</div>
			</>,
		);
	}

	async #shareCamera() {
		// TODO configure the constraints
		this.#publish.media = await navigator.mediaDevices.getUserMedia({
			video: true,
		});
	}

	async #shareScreen() {
		this.#publish.media = await navigator.mediaDevices.getDisplayMedia({ video: true });
	}

	setAttr(name: string, oldValue?: string, newValue?: string) {
		this.attributeChangedCallback(name, oldValue, newValue);
	}

	connectedCallback() {
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
			case "preview":
				if (value) {
					this.#preview.srcObject = this.#publish.media ?? null;
				} else {
					this.#preview.srcObject = null;
				}
				break;
		}
	}

	get url(): string | null {
		return this.getAttribute("url");
	}

	set url(value: string | null) {
		if (value === null || value === "") {
			this.removeAttribute("url");
		} else {
			this.setAttribute("url", value);
		}
	}

	get preview(): boolean {
		return this.getAttribute("preview") !== null;
	}

	set preview(value: boolean) {
		if (value) {
			this.setAttribute("preview", "");
		} else {
			this.removeAttribute("preview");
		}
	}
}

customElements.define("moq-publish", MoqPublishElement);

declare global {
	interface HTMLElementTagNameMap {
		"moq-publish": MoqPublishElement;
	}
}
