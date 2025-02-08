import * as Rust from "@rust";

import { type ConnectionStatus, convertConnectionStatus } from "../connection";
import { MoqElement, attribute, element, jsx } from "../util";
import type { Watch } from "../watch";

/// Live is fired once when all broadcasts have been discovered (on startup).
export type MeetAction = "join" | "leave" | "live";

@element("moq-meet")
export class Meet extends MoqElement {
	#rust: Rust.Meet;
	#broadcasts: Set<Watch> = new Set();

	#container: HTMLDivElement;

	@attribute
	accessor url = "";

	constructor() {
		super();

		this.#rust = new Rust.Meet();

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
		this.#rust.url = url !== "" ? url : null;
	}

	async #run() {
		for await (const [action, name] of this.members()) {
			if (action === "join") {
				this.#join(name);
			} else if (action === "leave") {
				this.#leave(name);
			}
		}
	}

	#join(name: string) {
		const watch = (<moq-watch id={`broadcast-${name}`} url={`${this.url}/${name}`} />) as Watch;

		this.#container.appendChild(watch);
		this.#broadcasts.add(watch);
		this.#updateGrid();
	}

	#leave(name: string) {
		const id = `#broadcast-${name}`;

		const watch = this.#container.querySelector(id) as Watch | null;
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

	async *connectionStatus(): AsyncGenerator<ConnectionStatus> {
		const status = this.#rust.status();
		for (;;) {
			const next = await status.connection();
			yield convertConnectionStatus(next);
		}
	}

	async *members(): AsyncGenerator<[MeetAction, string]> {
		const announced = this.#rust.announced();
		while (true) {
			const announce = await announced.next();
			if (announce === undefined) {
				return;
			}

			switch (announce.action) {
				case Rust.MeetAction.Join:
					yield ["join", announce.name];
					break;
				case Rust.MeetAction.Leave:
					yield ["leave", announce.name];
					break;
				case Rust.MeetAction.Live:
					yield ["live", ""];
					break;
				default: {
					const _exhaustive: never = announce.action;
					throw new Error(_exhaustive);
				}
			}
		}
	}
}

declare global {
	interface HTMLElementTagNameMap {
		"moq-meet": Meet;
	}
}

export default Meet;
