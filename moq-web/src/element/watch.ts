import { Watch } from "../watch";
import type { WatchState } from "../watch";

export class MoqWatchElement extends HTMLElement {
	#watch: Watch;

	static get observedAttributes() {
		return ["url", "paused", "volume"];
	}

	constructor() {
		super();

		this.#watch = new Watch();

		const states = this.#watch.state();
		(async () => {
			for await (const state of states) {
				this.dispatchEvent(new CustomEvent("moq-state", { detail: state }));
			}
		})();

		const shadow = this.attachShadow({ mode: "open" });
		shadow.innerHTML = `
			<style>
				:host {
					display: block;
					position: relative;
				}

				canvas {
					display: block;
					max-width: 100%;
					height: auto;
				}
			</style>
			<canvas>
		`;
	}

	connectedCallback() {
		const canvas = this.shadowRoot?.querySelector("canvas");
		if (!canvas) {
			throw new Error("canvas not found");
		}

		this.#watch.canvas = canvas as HTMLCanvasElement;

		for (const name of MoqWatchElement.observedAttributes) {
			const value = this.getAttribute(name);
			if (value !== undefined) {
				this.attributeChangedCallback(name, null, value);
			}
		}
	}

	disconnectedCallback() {
		this.#watch.canvas = null;
		this.#watch.close();
	}

	attributeChangedCallback(name: string, old: string | null, value: string | null) {
		if (old === value) {
			return;
		}

		switch (name) {
			case "url":
				this.#watch.url = value;
				break;
		}
	}

	get url(): string | null {
		return this.getAttribute("url");
	}

	set url(value: string | null) {
		if (value === null) {
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

	get error(): string | null {
		return this.#watch.error;
	}
}

customElements.define("moq-watch", MoqWatchElement);

declare global {
	interface HTMLElementTagNameMap {
		"moq-watch": MoqWatchElement;
	}

	interface GlobalEventHandlersEventMap {
		"moq-state": CustomEvent<WatchState>;
	}
}
