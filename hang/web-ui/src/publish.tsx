import { Broadcast, Device } from "@kixelated/hang/publish";

import { Context } from "./util/context";
import { element, MoqElement } from "./util/component";
import { jsx } from "./util/jsx";

import "@shoelace-style/shoelace/dist/components/radio-group/radio-group.js";
import "@shoelace-style/shoelace/dist/components/radio-button/radio-button.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";
import "@shoelace-style/shoelace/dist/components/tooltip/tooltip.js";

@element("hang-publish")
export class Publish extends MoqElement {
	#slot: HTMLSlotElement;
	#controls: HTMLDivElement;
	#status: HTMLDivElement;

	#broadcast: Broadcast;
	#context?: Context; // Cancel background tasks when not in the DOM.

	// The canvas element can be slotted, so we have to do extra work.
	#canvas?: HTMLCanvasElement;

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
			<div css={{ display: "flex", justifyContent: "center", gap: "8px" }}>
				<sl-radio-group>
					<sl-tooltip content="Publish your webcam." placement="bottom">
						<sl-radio-button onclick={() => this.#share("camera")}>
							<sl-icon slot="prefix" name="camera" label="Camera" />
						</sl-radio-button>
					</sl-tooltip>

					<sl-tooltip content="Publish a screen or window." placement="bottom">
						<sl-radio-button onclick={() => this.#share("screen")}>
							<sl-icon slot="prefix" name="display" label="Screen" />
						</sl-radio-button>
					</sl-tooltip>

					<sl-tooltip content="Publish nothing but don't leave the meeting." placement="bottom">
						<sl-radio-button onclick={() => this.#share("")}>
							<sl-icon slot="prefix" name="x" label="None" />
						</sl-radio-button>
					</sl-tooltip>
				</sl-radio-group>
			</div>
		) as HTMLDivElement;

		// Create a slot with a default canvas element.
		this.#slot = (
			<slot>
				<canvas />
			</slot>
		) as HTMLSlotElement;
		this.#slot.addEventListener("slotchange", () => this.#updateCanvas());

		this.#broadcast = new Broadcast();
		// TODO set the default canvas

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(this.#status);
		shadow.appendChild(this.#slot);
		shadow.appendChild(this.#controls);
	}

	connectedCallback() {
		this.#context = new Context();
		this.#run(this.#context);
	}

	disconnectedCallback() {
		this.#context?.cancel("removed from DOM");
	}

	#updateCanvas() {
		for (const element of this.#slot.assignedElements({ flatten: true })) {
			if (element instanceof HTMLCanvasElement) {
				this.#broadcast.canvas = element;
				return;
			}
		}

		throw new Error("Expected slotted <canvas> element.");
	}

	async #run(context: Context) {
		this.#status.replaceChildren(<sl-spinner />, "Initiailizing...");

		try {
			const status = this.#broadcast.status();

			for (;;) {
				const { done, value } = await Promise.race([status.next(), context.done]);
				if (done) {
					return;
				}

				switch (value) {
					case "disconnected":
						if (this.#broadcast.device !== "") {
							this.#status.replaceChildren("Disconnected");
						} else {
							this.#status.replaceChildren();
						}
						break;
					case "connecting":
						this.#status.replaceChildren(<sl-spinner />, "Connecting...");
						break;
					case "connected":
					case "live": // TODO live icon
					case "offline": // TODO offline icon?
						this.#status.replaceChildren();
						break;
					default: {
						value as never;
						throw new Error(`Unhandled state: ${value}`);
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

	#share(device: Device) {
		this.#broadcast.device = device;
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"hang-publish": Publish;
	}
}

export default Publish;
