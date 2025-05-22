import { signal } from "@kixelated/signals";
import { Show, render } from "solid-js/web";
import { PublishDevice } from "./broadcast";
import { PublishControls } from "./controls";
import { Publish } from "./publish";

export class PublishElement extends HTMLElement {
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
					<PublishControls lib={this.lib} />
				</Show>
			),
			this,
		);
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.lib.connection.url.set(newValue ? new URL(newValue) : undefined);
		} else if (name === "device") {
			this.lib.broadcast.device.set(newValue as PublishDevice);
		} else if (name === "audio") {
			this.lib.broadcast.audio.constraints.set(newValue !== undefined);
		} else if (name === "video") {
			this.lib.broadcast.video.constraints.set(newValue !== undefined);
		} else if (name === "controls") {
			this.#controls.set(newValue !== undefined);
		}
	}
}

customElements.define("hang-publish", PublishElement);

declare global {
	interface HTMLElementTagNameMap {
		"hang-publish": PublishElement;
	}
}
