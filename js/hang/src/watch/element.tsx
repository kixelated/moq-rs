import { Show, render } from "solid-js/web";
import { signal } from "../signals";
import { WatchControls } from "./controls";
import { Watch } from "./watch";

// An optional web component that wraps a <canvas>
export class WatchElement extends HTMLElement {
	static observedAttributes = ["url", "paused", "volume", "latency", "controls"];

	#controls = signal(false);

	lib: Watch;

	constructor() {
		super();

		const canvas = this.querySelector("canvas") as HTMLCanvasElement | undefined;

		// The broadcast path is relative to the connection URL.
		this.lib = new Watch({ video: { canvas }, broadcast: { path: "" } });

		// Create an element for controls if they want them.
		const controls = document.createElement("div");

		// Render the controls element.
		render(
			() => (
				<Show when={this.#controls.get()}>
					<WatchControls lib={this.lib} root={this} />
				</Show>
			),
			controls,
		);

		this.append(controls);
	}

	attributeChangedCallback(name: string, oldValue: string | undefined, newValue: string | undefined) {
		if (oldValue === newValue) {
			return;
		}

		if (name === "url") {
			this.lib.connection.url.set(newValue ? new URL(newValue) : undefined);
		} else if (name === "paused") {
			this.lib.video.paused.set(newValue !== undefined);
			this.lib.audio.paused.set(newValue !== undefined);
		} else if (name === "volume") {
			const volume = newValue ? Number.parseFloat(newValue) : 0.5;
			this.lib.audio.volume.set(volume);
		} else if (name === "latency") {
			const latency = newValue ? Number.parseInt(newValue) : 50;
			this.lib.video.latency.set(latency);
			this.lib.audio.latency.set(latency);
		} else if (name === "controls") {
			this.#controls.set(newValue !== undefined);
		}
	}

	// TODO Do this on disconnectedCallback?
	close() {
		this.lib.close();
	}
}

customElements.define("hang-watch", WatchElement);

declare global {
	interface HTMLElementTagNameMap {
		"hang-watch": WatchElement;
	}
}
