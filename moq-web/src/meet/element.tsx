import { Meet } from ".";

import { MoqElement, attribute, element, jsx } from "../element";
import type { WatchElement } from "../watch/element";

@element("moq-meet")
export class MeetElement extends MoqElement {
	public readonly lib: Meet;

	#container: HTMLDivElement;
	#broadcasts: Set<WatchElement> = new Set();

	@attribute
	accessor url = "";

	constructor() {
		super();

		this.lib = new Meet();

		const style = (
			<style>
				{`
				:host {
					display: block;
					position: relative;

					max-width: 100%;
					max-height: 100%;
				}
				`}
			</style>
		);

		this.#container = (
			<div
				css={{
					display: "grid",
					gap: "8px",
					maxWidth: "100%",
					maxHeight: "100%",
					placeItems: "center",
				}}
			/>
		) as HTMLDivElement;

		this.#run();

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(this.#container);
	}

	urlChange(url: string) {
		this.lib.url = url !== "" ? url : null;
	}

	async #run() {
		for await (const [action, name] of this.lib.members()) {
			if (action === "join") {
				this.#join(name);
			} else if (action === "leave") {
				this.#leave(name);
			}
		}
	}

	#join(name: string) {
		const watch = (
			<moq-watch
				id={`broadcast-${name}`}
				url={`${this.url}/${name}`}
				css={{ borderRadius: "0.5rem", overflow: "hidden" }}
			/>
		) as WatchElement;

		this.#container.appendChild(watch);
		this.#broadcasts.add(watch);
		this.#updateGrid();
	}

	#leave(name: string) {
		const id = `#broadcast-${name}`;

		const watch = this.#container.querySelector(id) as WatchElement | null;
		if (!watch) {
			console.warn(`Broadcast not found: ${id}`);
			return;
		}

		watch.remove();
		this.#broadcasts.delete(watch);
		this.#updateGrid();
	}

	#updateGrid() {
		if (this.#broadcasts.size === 0) {
			return;
		}

		// Calculate grid size (square root approximation)
		const cols = Math.ceil(Math.sqrt(this.#broadcasts.size));
		const rows = Math.ceil(this.#broadcasts.size / cols);

		this.#container.style.gridTemplateColumns = `repeat(${cols}, 1fr)`;
		this.#container.style.gridTemplateRows = `repeat(${rows}, 1fr)`;
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-meet": MeetElement;
	}
}

export default MeetElement;
