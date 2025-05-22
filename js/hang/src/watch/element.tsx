import { Signals, signal } from "@kixelated/signals";
import { Show, render } from "solid-js/web";
import { WatchControls } from "./controls";
import { Watch } from "./watch";

// An optional web component that wraps a <canvas>
export class WatchElement extends HTMLElement {
	static observedAttributes = ["url", "paused", "volume", "muted", "controls"];

	#controls = signal(false);

	lib: Watch;

	#signals = new Signals();

	constructor() {
		super();

		const canvas = this.querySelector("canvas") as HTMLCanvasElement | undefined;

		// The broadcast path is relative to the connection URL.
		this.lib = new Watch({ video: { canvas }, broadcast: { path: "" } });

		// Render the controls element.
		render(
			() => (
				<Show when={this.#controls.get()}>
					<WatchControls lib={this.lib} root={this} />
				</Show>
			),
			this,
		);

		// Optionally update attributes to match the library state.
		// This is kind of dangerous because it can create loops.
		this.#signals.effect(() => {
			const url = this.lib.connection.url.get();
			if (url) {
				this.setAttribute("url", url.toString());
			} else {
				this.removeAttribute("url");
			}
		});

		this.#signals.effect(() => {
			const muted = this.lib.audio.muted.get();
			if (muted) {
				this.setAttribute("muted", "");
			} else {
				this.removeAttribute("muted");
			}
		});

		this.#signals.effect(() => {
			const paused = this.lib.video.paused.get();
			if (paused) {
				this.setAttribute("paused", "");
			} else {
				this.removeAttribute("paused");
			}
		});

		this.#signals.effect(() => {
			const volume = this.lib.audio.volume.get();
			this.setAttribute("volume", volume.toString());
		});

		this.#signals.effect(() => {
			const controls = this.#controls.get();
			if (controls) {
				this.setAttribute("controls", "");
			} else {
				this.removeAttribute("controls");
			}
		});
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
		} else if (name === "muted") {
			this.lib.audio.muted.set(newValue !== undefined);
		} else if (name === "controls") {
			this.#controls.set(newValue !== undefined);
		}
	}

	// TODO Do this on disconnectedCallback?
	close() {
		this.lib.close();
		this.#signals.close();
	}
}

customElements.define("hang-watch", WatchElement);

declare global {
	interface HTMLElementTagNameMap {
		"hang-watch": WatchElement;
	}
}
