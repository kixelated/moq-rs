import * as Rust from "@rust";
import type { MoqWatchElement } from "..";

import { Element, attribute, element } from "./component";
import { jsx } from "./jsx";

import "@shoelace-style/shoelace/dist/components/spinner/spinner.js";
import "@shoelace-style/shoelace/dist/components/alert/alert.js";
import "@shoelace-style/shoelace/dist/components/icon/icon.js";

@element("moq-meet")
export class MoqMeetElement extends Element {
	#room: Rust.Room;
	#container: HTMLDivElement;
	#broadcasts: Set<MoqWatchElement> = new Set();
	#status: HTMLDivElement;

	@attribute
	accessor url = "";

	@attribute
	accessor controls = false;

	@attribute
	accessor status = false;

	constructor() {
		super();

		const style = (
			<style>
				{`
				:host {
					display: block;
					position: relative;

					max-width: 100%;
					max-height: 100%;
				}

				:host([status]) #status {
					display: flex;
					gap: 8px;
					justify-content: center;
					font-family: var(--sl-font-sans);
				}

				:host(:not([status])) #status  {
					display: none;
				}
				`}
			</style>
		);

		this.#status = (<div id="status" />) as HTMLDivElement;

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

		this.#room = new Rust.Room();
		const announced = this.#room.announced();
		this.#runAnnounced(announced).finally(() => announced.free());

		const shadow = this.attachShadow({ mode: "open" });
		shadow.appendChild(style);
		shadow.appendChild(this.#status);
		shadow.appendChild(this.#container);
	}

	urlChange(url: string) {
		this.#room.url = url;
	}

	controlsChange(value: boolean) {
		for (const broadcast of this.#broadcasts) {
			broadcast.controls = value;
		}
	}

	statusChange(value: boolean) {
		for (const broadcast of this.#broadcasts) {
			broadcast.status = value;
		}
	}

	async #runAnnounced(announced: Rust.RoomAnnounced) {
		this.#status.replaceChildren(<sl-spinner />, "Fetching Broadcasts...");

		let live = false;

		while (true) {
			const announce = await announced.next();
			if (!announce) {
				// TODO get error message
				this.#status.replaceChildren(
					<sl-alert variant="danger" open css={{ width: "100%" }}>
						<sl-icon slot="icon" name="exclamation-octagon" />
						<strong>Disconnected</strong>
						<br />
						{this.#room.error || "Unknown error"}
					</sl-alert>,
				);
				return;
			}

			this.#status.replaceChildren();

			switch (announce.action) {
				case Rust.RoomAction.Join:
					this.#join(announce.name);
					break;
				case Rust.RoomAction.Leave:
					this.#leave(announce.name);
					break;
				case Rust.RoomAction.Live:
					live = true;
					break;
			}

			if (live && this.#broadcasts.size === 0) {
				this.#status.replaceChildren("🦗 nobody is here 🦗");
			}
		}
	}

	#join(name: string) {
		const watch = (
			<moq-watch
				id={`broadcast-${name}`}
				url={`${this.url}/${name}`}
				controls={this.controls}
				status={this.status}
				css={{ borderRadius: "0.5rem", overflow: "hidden" }}
			/>
		) as MoqWatchElement;

		this.#container.appendChild(watch);
		this.#broadcasts.add(watch);
		this.#updateGrid();
	}

	#leave(name: string) {
		const id = `#broadcast-${name}`;

		const watch = this.#container.querySelector(id) as MoqWatchElement | null;
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
		"moq-meet": MoqMeetElement;
	}
}

export default MoqMeetElement;
