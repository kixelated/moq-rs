import { MoqElement, attribute, element } from "../element/component";
import { jsx } from "../element/jsx";

import { type RendererStatus, Watch } from ".";
import type { ConnectionStatus } from "../connection";

@element("moq-watch")
export class WatchElement extends MoqElement {
	public readonly lib: Watch;

	@attribute
	accessor url = "";

	@attribute
	accessor paused = false;

	@attribute
	accessor volume = 1;

	@attribute
	accessor latency = 0;

	constructor() {
		super();

		const style = (
			<style>
				{`
				:host {
					display: block;
					position: relative;
					overflow: hidden;

					max-width: 100%;
					max-height: 100%;

					justify-content: center;
				}
				`}
			</style>
		);

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);

		this.lib = new Watch();
		this.lib.canvas = shadow.appendChild(
			<canvas width={0} height={0} css={{ maxWidth: "100%", height: "auto" }} />,
		) as HTMLCanvasElement;

		// Set data- attributes and fire callbacks.
		this.#runConnectionStatus();
		this.#runRendererStatus();
	}

	urlChange(value: string) {
		this.lib.url = value !== "" ? value : null;
	}

	pausedChange(value: boolean) {
		this.lib.paused = value;
	}

	volumeChange(value: number) {
		this.lib.volume = value;
	}

	latencyChange(value: number) {
		this.lib.latency = value;
	}

	async #runConnectionStatus() {
		for await (const state of this.lib.connectionStatus()) {
			this.setAttribute("data-connection-status", state);
		}
	}

	async #runRendererStatus() {
		for await (const state of this.lib.rendererStatus()) {
			this.setAttribute("data-renderer-status", state);
		}
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-watch": WatchElement;
	}
}

class ConnectionStatusEvent extends CustomEvent<ConnectionStatus> {
	constructor(detail: ConnectionStatus) {
		super("moq-connection-status", { detail, bubbles: true, composed: true });
	}
}

class RendererStatusEvent extends CustomEvent<RendererStatus> {
	constructor(detail: RendererStatus) {
		super("moq-renderer-status", { detail, bubbles: true, composed: true });
	}
}

declare global {
	interface HTMLElementEventMap {
		"moq-connection-status": ConnectionStatusEvent;
		"moq-renderer-status": RendererStatusEvent;
	}
}

export default WatchElement;
