import { Meet } from ".";

import { Context, MoqElement, attribute, element, jsx } from "../util";

import "@shoelace-style/shoelace/dist/components/spinner/spinner.js";
import "@shoelace-style/shoelace/dist/components/alert/alert.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";

@element("moq-meet-ui")
export class MeetUi extends MoqElement {
	#slot: HTMLSlotElement;
	#status: HTMLDivElement;

	#inner: Meet | null = null;
	#context?: Context;

	constructor() {
		super();

		const style = (
			<style>
				{`
				:host {
					display: flex;
					flex-direction: column;

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

		this.#slot = (<slot>Unable to find `moq-meet` element.</slot>) as HTMLSlotElement;
		this.#slot.addEventListener("slotchange", () => this.#init());

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(this.#status);
		shadow.appendChild(this.#slot);
	}

	#init() {
		for (const element of this.#slot.assignedElements({ flatten: true })) {
			if (element instanceof Meet) {
				if (this.#context) {
					this.#context.cancel("removed from DOM");
				}

				this.#inner = element;
				this.#context = new Context();
				this.#run(element, this.#context);
				return;
			}
		}

		throw new Error("Expected <moq-meet> element.");
	}

	async #run(element: Meet, context: Context) {
		this.#status.replaceChildren(<sl-spinner />, "Initializing...");

		try {
			const status = element.connectionStatus();

			for (;;) {
				const { done, value } = await Promise.race([status.next(), context.done]);
				if (done) {
					return;
				}

				switch (value) {
					case "connecting":
						this.#status.replaceChildren(<sl-spinner />, "Connecting...");
						break;
					case "connected":
						this.#status.replaceChildren(<sl-spinner />, "Searching...");
						break;
					case "disconnected":
						this.#status.replaceChildren("Disconnected");
						break;
					case "live":
						this.#status.replaceChildren();
						break;
					case "offline":
						this.#status.replaceChildren("🦗 nobody is here 🦗");
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
					<strong>Disconnected</strong>
					<br />
					{err || "Unknown error"}
				</sl-alert>,
			);
		}
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-meet-ui": MeetUi;
	}
}

export default MeetUi;
