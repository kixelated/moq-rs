import { signal } from "@kixelated/signals";
import { Show, render } from "solid-js/web";
import { Device } from "./broadcast";
import { Controls } from "./controls";
import { Publish } from "./publish";

export default class HangPublish extends HTMLElement {
	static observedAttributes = ["url", "device", "audio", "video", "controls"];

	#controls = signal(false);

	lib: Publish;

	constructor() {
		super();

		const preview = this.querySelector("video") as HTMLVideoElement | undefined;

		// The broadcast path is "" because it's relative to the connection URL.
		this.lib = new Publish({ preview, broadcast: { path: "" } });

		// Render the controls element.
		render(
			() => (
				<Show when={this.#controls.get()}>
					<Controls lib={this.lib} />
				</Show>
			),
			this,
		);
	}

	attributeChangedCallback(name: string, _oldValue: string | null, newValue: string | null) {
		if (name === "url") {
			this.lib.connection.url.set(newValue ? new URL(newValue) : undefined);
		} else if (name === "device") {
			this.lib.broadcast.device.set(newValue as Device);
		} else if (name === "audio") {
			this.lib.broadcast.audio.constraints.set(newValue !== null);
		} else if (name === "video") {
			this.lib.broadcast.video.constraints.set(newValue !== null);
		} else if (name === "controls") {
			this.#controls.set(newValue !== null);
		}
	}
}

customElements.define("hang-publish", HangPublish);

declare global {
	interface HTMLElementTagNameMap {
		"hang-publish": HangPublish;
	}
}
