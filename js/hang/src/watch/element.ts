import { Connection } from "../connection";
import { Signals, signal } from "../signals";
import { AudioEmitter } from "./audio";
import { Broadcast } from "./broadcast";
import { VideoRenderer } from "./video";

// A custom element that renders to a canvas.
export class Watch extends HTMLElement {
	static observedAttributes = ["url", "name", "paused", "muted", "latency"];

	connection = new Connection();
	broadcast = new Broadcast({ connection: this.connection });

	video = this.broadcast.video;
	videoRenderer = new VideoRenderer({ source: this.video });

	audio = this.broadcast.audio;
	audioEmitter = new AudioEmitter({ source: this.audio });

	paused = signal(false);
	muted = signal(false);
	volume = signal(0.5);

	// TODO this is pretty high because the BBB audio stutters; fix that.
	latency = signal(100);

	// Detect when the element is no longer visible.
	#visible = signal(true);
	readonly visible = this.#visible.readonly();

	#signals?: Signals;

	constructor() {
		super();

		const canvas = document.createElement("canvas");
		canvas.style.width = "100%";
		canvas.style.height = "auto";

		const slot = document.createElement("slot");
		slot.addEventListener("slotchange", () => {
			for (const el of slot.assignedElements({ flatten: true })) {
				if (el instanceof HTMLCanvasElement) {
					this.videoRenderer.canvas.set(el);
					return;
				}
			}

			this.videoRenderer.canvas.set(undefined);
		});

		slot.appendChild(canvas);
		this.videoRenderer.canvas.set(canvas);

		const style = document.createElement("style");
		style.textContent = `
			:host {
				display: flex;
				align-items: center;
				justify-content: center;
			}
		`;

		this.attachShadow({ mode: "open" }).append(style, slot);

		// Detect when the element is no longer visible.
		const observer = new IntersectionObserver(
			(entries) => {
				for (const entry of entries) {
					this.#visible.set(entry.isIntersecting);
				}
			},
			{
				threshold: 0.01, // fire when even a small part is visible
			},
		);

		observer.observe(this);
	}

	connectedCallback() {
		this.#signals = new Signals();

		const volume = this.#signals.derived(() => (this.muted.get() ? 0 : this.volume.get()));

		this.#signals.effect(() => {
			const enabled = this.paused.get() ? false : volume.get() > 0;
			this.audioEmitter.volume.set(volume.get());
			this.audio.enabled.set(enabled);
			this.audioEmitter.paused.set(this.paused.get());
		});

		this.#signals.effect(() => {
			const enabled = this.visible.get() && !this.paused.get();
			this.video.enabled.set(enabled);
			this.videoRenderer.paused.set(this.paused.get());
		});

		this.#signals.effect(() => {
			this.audioEmitter.latency.set(this.latency.get());
			this.video.latency.set(this.latency.get());
		});
	}

	disconnectedCallback() {
		this.#signals?.close();
	}

	attributeChangedCallback(name: string, oldValue: string | undefined, newValue: string | undefined) {
		if (oldValue === newValue) {
			return;
		}

		if (name === "url") {
			this.connection.url.set(newValue ? new URL(newValue) : undefined);
		} else if (name === "name") {
			this.broadcast.path.set(newValue);
		} else if (name === "paused") {
			this.paused.set(newValue !== undefined);
		} else if (name === "muted") {
			this.muted.set(newValue !== undefined);
		}
	}

	// TODO Do this on disconnectedCallback?
	close() {
		this.#signals?.close();

		this.connection.close();
		this.broadcast.close();
		this.audio.close();
		this.video.close();
	}
}

customElements.define("hang-watch", Watch);

declare global {
	interface HTMLElementTagNameMap {
		"hang-watch": Watch;
	}
}
