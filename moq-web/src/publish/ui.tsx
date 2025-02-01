import { PublishElement } from "./element";

import { MoqElement, attribute, element } from "../element/component";

import { jsx } from "../element/jsx";

import "@shoelace-style/shoelace/dist/components/radio-group/radio-group.js";
import "@shoelace-style/shoelace/dist/components/radio-button/radio-button.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";
import "@shoelace-style/shoelace/dist/components/tooltip/tooltip.js";

type MediaKind = "camera" | "screen" | "";

@element("moq-publish-ui")
export class PublishUi extends MoqElement {
	#inner: PublishElement;

	#controls: HTMLDivElement;
	#status: HTMLDivElement;

	constructor() {
		super();

		this.#inner = new PublishElement();

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

		this.#status = (
			<div
				css={{
					display: "flex",
					gap: "8px",
					justifyContent: "center",
					fontFamily: "var(--sl-font-sans)",
				}}
			/>
		) as HTMLDivElement;

		this.#controls = (
			<div css={{ display: "none", justifyContent: "center", gap: "8px" }}>
				<sl-radio-group>
					<sl-tooltip content="Publish your webcam." placement="bottom">
						<sl-radio-button
							onclick={() => {
								this.#inner.media = "camera";
							}}
						>
							<sl-icon slot="prefix" name="camera" label="Camera" />
						</sl-radio-button>
					</sl-tooltip>
					<sl-tooltip content="Publish a screen or window." placement="bottom">
						<sl-radio-button
							onclick={() => {
								this.#inner.media = "screen";
							}}
						>
							<sl-icon slot="prefix" name="display" label="Screen" />
						</sl-radio-button>
					</sl-tooltip>
					<sl-tooltip content="Publish nothing and leave the meeting." placement="bottom">
						<sl-radio-button
							onclick={() => {
								this.#inner.media = "";
							}}
						>
							<sl-icon slot="prefix" name="x" label="None" />
						</sl-radio-button>
					</sl-tooltip>
				</sl-radio-group>
			</div>
		) as HTMLDivElement;

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(this.#status);
		shadow.appendChild(this.#inner);
		shadow.appendChild(this.#controls);

		this.#runStatus();
	}

	async #runStatus() {
		this.#status.replaceChildren(<sl-spinner />, "Initiailizing...");

		try {
			for await (const status of this.#inner.lib.connectionStatus()) {
				switch (status) {
					case "connecting":
						this.#status.replaceChildren(<sl-spinner />, "Connecting...");
						break;
					case "disconnected":
						this.#status.replaceChildren("Disconnected");
						break;
					case "connected":
					case "live": // TODO live icon
					case "offline": // TODO offline icon?
						this.#status.replaceChildren();
						break;
					default: {
						const _exhaustive: never = status;
						throw new Error(`Unhandled state: ${_exhaustive}`);
					}
				}
			}
		} catch (err) {
			this.#status.replaceChildren(
				<sl-alert variant="danger" open css={{ width: "100%" }}>
					<sl-icon slot="icon" name="exclamation-octagon" />
					<strong>Error</strong>
					<br />
					{err}
				</sl-alert>,
			);
		}
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-publish": PublishElement;
	}
}

export default PublishElement;
