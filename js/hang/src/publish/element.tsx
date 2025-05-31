import { Signals, signal } from "@kixelated/signals";
import { Show, render } from "solid-js/web";
import { Connection } from "../connection";
import { Broadcast, Device } from "./broadcast";
import { Controls } from "./controls";

export default class HangPublish extends HTMLElement {
	static observedAttributes = ["url", "device", "audio", "video", "controls"];

	#controls = signal(false);

	connection: Connection;
	broadcast: Broadcast;

	#signals = new Signals();

	constructor() {
		super();

		const preview = this.querySelector("video") as HTMLVideoElement | undefined;

		this.connection = new Connection();
		this.broadcast = new Broadcast(this.connection);

		// Only publish when we have media available.
		this.#signals.effect(() => {
			const audio = this.broadcast.audio.media.get();
			const video = this.broadcast.video.media.get();
			this.broadcast.enabled.set(!!audio || !!video);
		});

		this.#signals.effect(() => {
			const media = this.broadcast.video.media.get();
			if (!media || !preview) return;

			preview.srcObject = new MediaStream([media]) ?? null;
			return () => {
				preview.srcObject = null;
			};
		});

		// Render the controls element.
		render(
			() => (
				<Show when={this.#controls.get()}>
					<Controls broadcast={this.broadcast} />
				</Show>
			),
			this,
		);
	}

	attributeChangedCallback(name: string, _oldValue: string | null, newValue: string | null) {
		if (name === "url") {
			this.connection.url.set(newValue ? new URL(newValue) : undefined);
		} else if (name === "device") {
			this.broadcast.device.set(newValue as Device);
		} else if (name === "audio") {
			this.broadcast.audio.constraints.set(newValue !== null);
		} else if (name === "video") {
			this.broadcast.video.constraints.set(newValue !== null);
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
