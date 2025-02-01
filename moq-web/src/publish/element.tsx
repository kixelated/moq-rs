import { Publish } from ".";

import { MoqElement, attribute, element } from "../element/component";

import { jsx } from "../element/jsx";

import "@shoelace-style/shoelace/dist/components/radio-group/radio-group.js";
import "@shoelace-style/shoelace/dist/components/radio-button/radio-button.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";
import "@shoelace-style/shoelace/dist/components/tooltip/tooltip.js";

type MediaKind = "camera" | "screen" | "";

@element("moq-publish")
export class PublishElement extends MoqElement {
	public readonly lib: Publish;

	// Optionally preview our media stream.
	#preview: HTMLVideoElement;

	@attribute
	accessor url = "";

	@attribute
	accessor media: MediaKind = "";

	@attribute
	accessor preview = false;

	constructor() {
		super();

		const style = (
			<style>
				{`
				:host {
					display: block;
					position: relative;

					max-width: 100%;
					max-height: 100%;
				}
				`}
			</style>
		);

		this.lib = new Publish();

		this.#preview = (
			<video
				css={{
					objectFit: "contain",
					maxWidth: "100%",
					maxHeight: "100%",
					display: "none",
				}}
				autoplay
			/>
		) as HTMLVideoElement;

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(this.#preview);
	}

	urlChange(url: string) {
		this.lib.url = url !== "" ? url : null;
	}

	async mediaChange(value: MediaKind) {
		let media: MediaStream | null;
		switch (value) {
			case "camera":
				await this.lib.shareCamera();
				break;
			case "screen":
				await this.lib.shareScreen();
				break;
			case "":
				this.lib.unshare();
				break;
			default: {
				const _: never = value;
				throw new Error(`Invalid media kind: ${value}`);
			}
		}

		this.#updatePreview();
	}

	previewChange(value: boolean) {
		this.#preview.style.display = value ? "" : "none";
		this.#updatePreview();
	}

	#updatePreview() {
		this.#preview.srcObject = this.preview && this.lib.media ? this.lib.media : null;
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-publish": PublishElement;
	}
}

export default PublishElement;
