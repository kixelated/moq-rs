import { Show, render } from "solid-js/web";
import { signal } from "../signals";
import { Device } from "./broadcast";
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

		// Create an element for controls if they want them.
		const controls = document.createElement("div");

		// Render the controls element.
		render(
			() => (
				<Show when={this.#controls.get()}>
					<PublishControls lib={this.lib} />
				</Show>
			),
			controls,
		);

		this.append(controls);
	}

	attributeChangedCallback(name: string, _oldValue: string | undefined, newValue: string | undefined) {
		if (name === "url") {
			this.lib.connection.url.set(newValue ? new URL(newValue) : undefined);
		} else if (name === "device") {
			this.lib.broadcast.device.set(newValue as Device);
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
