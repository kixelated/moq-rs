import { Watch } from "..";
import type { WatchState } from "..";

import { attribute, element, Element } from "./component";
import { jsx } from "./jsx";

@element("moq-watch")
export class MoqWatch extends Element {
	#watch: Watch;
	#canvas: OffscreenCanvas;

	@attribute
	accessor url = "";

	@attribute
	accessor paused = false;

	@attribute
	accessor volume = 1;

	constructor() {
		super();

		const style = (
			<style>
				{`
				:host {
					display: block;
					overflow: hidden;
					position: relative;
				}
				`}
			</style>
		);

		const canvas = (
			<canvas css={{ display: "block", maxWidth: "100%", height: "auto" }} width={0} height={0} />
		) as HTMLCanvasElement;

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(canvas);

		this.#canvas = canvas.transferControlToOffscreen();

		// We initialize the Watch here before getting added to the DOM so we could preload.
		this.#watch = new Watch();
		this.#watch.canvas = this.#canvas;
	}

	urlChange(value: string) {
		if (this.#watch) {
			this.#watch.url = value;
		}
	}

	pausedChange(value: boolean) {
		if (this.#watch) {
			this.#watch.paused = value;
		}
	}

	volumeChange(value: number) {
		if (this.#watch) {
			this.#watch.volume = value;
		}
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-watch": MoqWatch;
	}

	interface GlobalEventHandlersEventMap {
		"moq-watch-state": CustomEvent<WatchState>;
	}
}

export default MoqWatch;
