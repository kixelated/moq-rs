import * as Moq from "..";

export class MoqWatchElement extends HTMLElement {
	#watch: Moq.Watch;

	static get observedAttributes() {
		return ["url"];
	}

	constructor() {
		super();

		this.#watch = new Moq.Watch();

		this.#watch.on_state((state: Moq.WatchState) => {
			this.dispatchEvent(new CustomEvent("moq-state", { detail: state }));
		});

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

	setAttr(name: string, oldValue?: string, newValue?: string) {
		this.attributeChangedCallback(name, oldValue, newValue);
	}

	connectedCallback() {
		const canvas = this.shadowRoot?.querySelector("canvas");
		if (!canvas) {
			throw new Error("canvas not found");
		}

		this.#watch.canvas = canvas as HTMLCanvasElement;

		for (const name of MoqWatchElement.observedAttributes) {
			const value = this.getAttribute(name) ?? undefined;
			if (value !== undefined) {
				this.attributeChangedCallback(name, undefined, value);
			}
		}
	}

	disconnectedCallback() {
		this.#watch.canvas = undefined;
		this.#watch.close();
	}

	attributeChangedCallback(name: string, old?: string, value?: string) {
		if (old === value) {
			return;
		}

		switch (name) {
			case "url":
				this.#watch.url = value ?? undefined;
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

	get error(): string | undefined {
		return this.#watch.error;
	}
}

customElements.define("moq-watch", MoqWatchElement);

declare global {
	interface HTMLElementTagNameMap {
		"moq-watch": MoqWatchElement;
	}

	interface GlobalEventHandlersEventMap {
		"moq-state": CustomEvent<Moq.WatchState>;
	}
}
