import { Meet } from ".";

import { MoqElement, attribute, element } from "../element/component";
import { jsx } from "../element/jsx";

import "@shoelace-style/shoelace/dist/components/spinner/spinner.js";
import "@shoelace-style/shoelace/dist/components/alert/alert.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";

@element("moq-meet-ui")
export class MeetUi extends MoqElement {
	#meet: Meet;
	#status: HTMLDivElement;

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

		this.#meet = new Meet();

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(this.#status);
		shadow.appendChild(this.#meet);

		this.#runStatus();
	}

	async #runStatus() {
		this.#status.replaceChildren(<sl-spinner />, "Initializing...");

		try {
			for await (const status of this.#meet.connectionStatus()) {
				switch (status) {
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
						const _exhaustive: never = status;
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
