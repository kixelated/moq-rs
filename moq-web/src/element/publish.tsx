import * as Moq from "..";
import { element, attribute, Element } from "./component";

import { jsx } from "./jsx";

@element("moq-publish")
export class MoqPublish extends Element {
	#publish: Moq.Publish;
	#preview: HTMLVideoElement;
	#media: MediaStream | null = null;

	@attribute
	accessor url = "";

	@attribute
	accessor media: "camera" | "screen" | "" = "";

	@attribute
	accessor preview = false;

	constructor() {
		super();

		const style = (
			<style>
				{`
				:host {
					display: block;
					overflow: hidden;
					position: relative;
				}
				`}
			</style>
		);

		this.#publish = new Moq.Publish();
		this.#preview = (
			<video css={{ objectFit: "contain", maxWidth: "100%", maxHeight: "100%", display: "none" }} autoplay />
		) as HTMLVideoElement;

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(this.#preview);
	}

	urlChange(value: string) {
		this.#publish.url = value;
	}

	mediaChange(value: string) {
		if (value !== "camera" && value !== "screen" && value !== "") {
			throw new Error(`Invalid media: ${value}`);
		}

		this.#publish.media = value === "" ? null : this.#media;
	}

	previewChange(value: boolean) {
		this.#preview.style.display = value ? "" : "none";
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-publish": MoqPublish;
	}
}
