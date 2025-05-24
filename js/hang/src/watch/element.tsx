import { Signals, signal } from "@kixelated/signals";
import { Show, render } from "solid-js/web";
import { Connection } from "../connection";
import { AudioEmitter } from "./audio";
import { Broadcast } from "./broadcast";
import { Controls } from "./controls";
import { VideoRenderer } from "./video";

// An optional web component that wraps a <canvas>
export default class HangWatch extends HTMLElement {
	static observedAttributes = ["url", "paused", "volume", "muted", "controls"];

	#controls = signal(false);

	// You can construct these manually if you want to use the library without the web component.
	// However be warned that the API is still in flux and may change.
	connection: Connection;
	broadcast: Broadcast;
	video: VideoRenderer;
	audio: AudioEmitter;

	#signals = new Signals();

	constructor() {
		super();

		const canvas = this.querySelector("canvas") as HTMLCanvasElement | undefined;

		this.connection = new Connection();
		this.broadcast = new Broadcast(this.connection);
		this.video = new VideoRenderer(this.broadcast.video, { canvas });
		this.audio = new AudioEmitter(this.broadcast.audio);

		// Render the controls element.
		render(
			() => (
				<Show when={this.#controls.get()}>
					<Controls broadcast={this.broadcast} video={this.video} audio={this.audio} root={this} />
				</Show>
			),
			this,
		);

		// Optionally update attributes to match the library state.
		// This is kind of dangerous because it can create loops.
		this.#signals.effect(() => {
			const url = this.connection.url.get();
			if (url) {
				this.setAttribute("url", url.toString());
			} else {
				this.removeAttribute("url");
			}
		});

		this.#signals.effect(() => {
			const muted = this.audio.muted.get();
			if (muted) {
				this.setAttribute("muted", "");
			} else {
				this.removeAttribute("muted");
			}
		});

		this.#signals.effect(() => {
			const paused = this.video.paused.get();
			if (paused) {
				this.setAttribute("paused", "true");
			} else {
				this.removeAttribute("paused");
			}
		});

		this.#signals.effect(() => {
			const volume = this.audio.volume.get();
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

		this.#signals.effect(() => {
			// Don't download audio if we're muted or paused.
			const paused = this.video.paused.get() || this.audio.muted.get();
			this.audio.paused.set(paused);
		});
	}

	attributeChangedCallback(name: string, oldValue: string | null, newValue: string | null) {
		if (oldValue === newValue) {
			return;
		}

		if (name === "url") {
			this.connection.url.set(newValue ? new URL(newValue) : undefined);
		} else if (name === "paused") {
			this.video.paused.set(newValue !== null);
		} else if (name === "volume") {
			const volume = newValue ? Number.parseFloat(newValue) : 0.5;
			this.audio.volume.set(volume);
		} else if (name === "muted") {
			this.audio.muted.set(newValue !== null);
		} else if (name === "controls") {
			this.#controls.set(newValue !== null);
		}
	}

	// TODO Do this on disconnectedCallback?
	close() {
		this.connection.close();
		this.broadcast.close();
		this.video.close();
		this.audio.close();
		this.#signals.close();
	}
}

customElements.define("hang-watch", HangWatch);

declare global {
	interface HTMLElementTagNameMap {
		"hang-watch": HangWatch;
	}
}
