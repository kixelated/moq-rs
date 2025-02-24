import { Publish, type PublishDevice } from ".";

import { Context, MoqElement, element, jsx } from "../util";

import "@shoelace-style/shoelace/dist/components/radio-group/radio-group.js";
import "@shoelace-style/shoelace/dist/components/radio-button/radio-button.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";
import "@shoelace-style/shoelace/dist/components/tooltip/tooltip.js";

@element("moq-publish-ui")
export class PublishUi extends MoqElement {
	#slot: HTMLSlotElement;
	#controls: HTMLDivElement;
	#status: HTMLDivElement;

	// The Publish element can be slotted, so we have to do extra work.
	#inner?: Publish;
	#context?: Context;

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

		this.#slot = (<slot>Unable to find `moq-publish` element.</slot>) as HTMLSlotElement;
		this.#slot.addEventListener("slotchange", () => this.#init());

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(this.#status);
		shadow.appendChild(this.#slot);
		shadow.appendChild(this.#controls);
	}

	#init() {
		for (const element of this.#slot.assignedElements({ flatten: true })) {
			if (element instanceof Publish) {
				if (this.#context) {
					this.#context.cancel("removed from DOM");
				}

				this.#inner = element;
				this.#context = new Context();
				this.#run(element, this.#context);
				return;
			}
		}

		throw new Error("Expected <moq-publish> element.");
	}

	async #run(inner: Publish, context: Context) {
		this.#status.replaceChildren(<sl-spinner />, "Initiailizing...");

		try {
			const connection = inner.connectionStatus();

			for (;;) {
				const { done, value } = await Promise.race([connection.next(), context.done]);
				if (done) {
					return;
				}

				switch (value) {
					case "disconnected":
						if (this.#inner?.device !== "") {
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
						const _exhaustive: never = value;
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

	#share(device: PublishDevice) {
		if (!this.#inner) {
			return;
		}

		this.#inner.device = device;
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-publish-ui": PublishUi;
	}
}

export default PublishUi;
