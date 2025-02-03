import * as Rust from "@rust";

import { Element, attribute, element } from "./component";

import { jsx } from "./jsx";

import "@shoelace-style/shoelace/dist/components/radio-group/radio-group.js";
import "@shoelace-style/shoelace/dist/components/radio-button/radio-button.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";
import "@shoelace-style/shoelace/dist/components/tooltip/tooltip.js";

type MediaKind = "camera" | "screen" | "";

@element("moq-publish")
export class MoqPublishElement extends Element {
	#publish: Rust.Publish;

	// Optional preview (pre-encoding)
	#preview: HTMLVideoElement;

	// Optional controls
	#controls: HTMLDivElement;

	// Optional status dialog
	#status: HTMLDivElement;

	@attribute
	accessor url = "";

	@attribute
	accessor media: MediaKind = "";

	@attribute
	accessor preview = false;

	@attribute
	accessor controls = false;

	@attribute
	accessor status = false;

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

				:host([status]) #status {
					display: flex;
					gap: 8px;
					justify-content: center;
					font-family: var(--sl-font-sans);
				}

				:host(:not([status])) #status  {
					display: none;
				}
				`}
			</style>
		);

		this.#status = (<div id="status" />) as HTMLDivElement;

		this.#controls = (
			<div css={{ display: "none", justifyContent: "center", gap: "8px" }}>
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
					<sl-tooltip content="Publish nothing and leave the meeting." placement="bottom">
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

		this.#publish = new Rust.Publish();
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
		shadow.appendChild(this.#status);
		shadow.appendChild(this.#controls);
		shadow.appendChild(this.#preview);

		this.#runStatus();
	}

	urlChange(url: string) {
		this.#updateConnect();
	}

	async mediaChange(value: MediaKind) {
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
			default: {
				const _: never = value;
				throw new Error(`Invalid media kind: ${value}`);
			}
		}

		this.#publish.media = media;
		this.#preview.srcObject = media;
		this.#updateConnect();
	}

	previewChange(value: boolean) {
		this.#preview.style.display = value ? "" : "none";
	}

	controlsChange(value: boolean) {
		this.#controls.style.display = value ? "flex" : "none";
	}

	#updateConnect() {
		const media = this.media;
		const url = this.url;

		// Connect when the user has selected a url and media source.
		if (media && url) {
			this.#publish.url = url;
		} else {
			this.#publish.url = null;
		}
	}

	async #runStatus() {
		this.#status.replaceChildren(<sl-spinner />, "Loading WASM Worker...");

		const states = await this.#publish.states();
		while (true) {
			const next = await states.next();
			if (next === undefined) {
				return;
			}

			switch (next) {
				case Rust.PublishState.Connecting:
					this.#status.replaceChildren(<sl-spinner />, "Connecting to Server...");
					break;
				case Rust.PublishState.Error: {
					const err = this.#publish.error || "unknown";
					this.#status.replaceChildren(
						<sl-alert variant="danger" open css={{ width: "100%" }}>
							<sl-icon slot="icon" name="exclamation-octagon" />
							<strong>Error</strong>
							<br />
							{err}
						</sl-alert>,
					);

					break;
				}
				case Rust.PublishState.Idle:
				case Rust.PublishState.Connected:
				case Rust.PublishState.Live: // TODO live icon
					this.#status.replaceChildren();
					break;
				default: {
					const _exhaustive: never = next;
					throw new Error(`Unhandled state: ${_exhaustive}`);
				}
			}
		}
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-publish": MoqPublishElement;
	}
}

export default MoqPublishElement;
