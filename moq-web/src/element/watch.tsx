import { Watch } from "..";
import type { WatchState } from "..";

import { jsx } from "./jsx";

const observedAttributes = ["url", "paused", "volume"] as const;
type ObservedAttribute = (typeof observedAttributes)[number];

export class MoqWatchElement extends HTMLElement {
	#watch: Watch | null;
	#canvas: OffscreenCanvas;

	static get observedAttributes() {
		return observedAttributes;
	}

	constructor() {
		super();

		const canvas = (<canvas css={{ display: "block", maxWidth: "100%", height: "auto" }} />) as HTMLCanvasElement;

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(canvas);

		this.#canvas = canvas.transferControlToOffscreen();

		// We initialize the Watch here before getting added to the DOM so we could preload.
		this.#watch = new Watch();
		this.#watch.canvas = this.#canvas;
	}

	connectedCallback() {
		// If we were disconnected, we need to reinitialize the Watch.
		if (this.#watch === null) {
			this.#watch = new Watch();
			this.#watch.canvas = this.#canvas;
		}

		const states = this.#watch.state();
		(async () => {
			try {
				for await (const state of states) {
					this.dispatchEvent(new CustomEvent("moq-watch-state", { detail: state }));
				}
			} catch (err) {
				// Used to clean up the WatchState so we don't leak memory.
				states.throw(err);
			}
		})();

		for (const name of MoqWatchElement.observedAttributes) {
			const value = this.getAttribute(name);
			if (value !== undefined) {
				this.attributeChangedCallback(name, null, value);
			}
		}
	}

	disconnectedCallback() {
		this.#watch?.free();
		this.#watch = null;
	}

	attributeChangedCallback(name: ObservedAttribute, old: string | null, value: string | null) {
		// Not readded to the DOM yet.
		if (this.#watch === null) {
			return;
		}

		if (old === value) {
			return;
		}

		switch (name) {
			case "url":
				this.#watch.url = value;
				break;
			case "paused":
				// TODO
				break;
			case "volume":
				// TODO
				break;
			default: {
				// Exhaustiveness check ensures all attributes are handled
				const _exhaustive: never = name;
				throw new Error(`Unhandled attribute: ${_exhaustive}`);
			}
		}
	}

	get url(): string | null {
		return this.getAttribute("url");
	}

	set url(value: string | null) {
		if (value === null || value === "") {
			this.removeAttribute("url");
		} else {
			this.setAttribute("url", value);
		}
	}

	get paused(): boolean {
		return this.getAttribute("paused") !== null;
	}

	set paused(value: boolean) {
		if (value) {
			this.setAttribute("paused", "");
		} else {
			this.removeAttribute("paused");
		}
	}

	get volume(): number {
		return Number.parseFloat(this.getAttribute("volume") ?? "1");
	}

	set volume(value: number) {
		this.setAttribute("volume", value.toString());
	}
}

customElements.define("moq-watch", MoqWatchElement);

declare global {
	interface HTMLElementTagNameMap {
		"moq-watch": MoqWatchElement;
	}

	interface GlobalEventHandlersEventMap {
		"moq-watch-state": CustomEvent<WatchState>;
	}
}
