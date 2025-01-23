import * as Moq from "..";
import { element, attribute, Element } from "./component";

import { jsx } from "./jsx";

import "@shoelace-style/shoelace/dist/themes/light.css";
import "@shoelace-style/shoelace/dist/themes/dark.css";
import "@shoelace-style/shoelace/dist/components/radio-group/radio-group.js";
import "@shoelace-style/shoelace/dist/components/radio-button/radio-button.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";
import "@shoelace-style/shoelace/dist/components/tooltip/tooltip.js";

@element("moq-publish")
export class MoqPublish extends Element {
	#publish: Moq.Publish;
	#preview: HTMLVideoElement;
	#controls: HTMLDivElement;

	@attribute
	accessor url = "";

	@attribute
	accessor media: "camera" | "screen" | "" = "";

	@attribute
	accessor preview = false;

	@attribute
	accessor controls = false;

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

		this.#controls = (
			<div css={{ display: "none", justifyContent: "center" }}>
				<sl-radio-group>
					<sl-tooltip content="Publish your webcam." placement="bottom">
						<sl-radio-button
							onclick={() => {
								this.media = "camera";
							}}
						>
							<sl-icon slot="prefix" name="camera" label="Camera" />
						</sl-radio-button>
					</sl-tooltip>
					<sl-tooltip content="Publish a screen or window." placement="bottom">
						<sl-radio-button
							onclick={() => {
								this.media = "screen";
							}}
						>
							<sl-icon slot="prefix" name="display" label="Screen" />
						</sl-radio-button>
					</sl-tooltip>
					<sl-tooltip content="Publish nothing (for now), but still join the meeting." placement="bottom">
						<sl-radio-button
							onclick={() => {
								this.media = "";
							}}
						>
							<sl-icon slot="prefix" name="x" label="None" />
						</sl-radio-button>
					</sl-tooltip>
				</sl-radio-group>
			</div>
		) as HTMLDivElement;

		this.#publish = new Moq.Publish();
		this.#preview = (
			<video css={{ objectFit: "contain", maxWidth: "100%", maxHeight: "100%", display: "none" }} autoplay />
		) as HTMLVideoElement;

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(this.#controls);
		shadow.appendChild(this.#preview);
	}

	urlChange(value: string) {
		this.#publish.url = value;
	}

	async mediaChange(value: string) {
		let media: MediaStream | null;
		switch (value) {
			case "camera":
				media = await navigator.mediaDevices.getUserMedia({ video: true });
				break;
			case "screen":
				media = await navigator.mediaDevices.getDisplayMedia({ video: true });
				break;
			case "":
				for (const track of this.#publish.media?.getTracks() || []) {
					track.stop();
				}
				media = null;
				break;
			default:
				throw new Error(`Invalid media kind: ${value}`);
		}

		this.#publish.media = media;
		this.#preview.srcObject = media;
	}

	previewChange(value: boolean) {
		this.#preview.style.display = value ? "" : "none";
	}

	controlsChange(value: boolean) {
		this.#controls.style.display = value ? "flex" : "none";
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-publish": MoqPublish;
	}
}
